#!/usr/bin/env python3
"""Tag release tooling. Python >=3.11, standard library only; no service/database writes."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from urllib.parse import quote
import zipfile

TARGETS = {
    'x86_64-unknown-linux-gnu': 'tar.gz',
    'x86_64-pc-windows-msvc': 'zip',
    'x86_64-apple-darwin': 'tar.gz',
    'aarch64-apple-darwin': 'tar.gz',
}
TAG = re.compile(r'v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-(alpha|beta|rc)\.(0|[1-9][0-9]*))?')


def parse_tag(tag):
    if not TAG.fullmatch(tag):
        raise ValueError('Expected vX.Y.Z or vX.Y.Z-{alpha,beta,rc}.N; no leading zeros')
    return tag[1:], '-' in tag


def validate(root, tag):
    version, prerelease = parse_tag(tag)
    with (root / 'Cargo.toml').open('rb') as file:
        manifest = tomllib.load(file)
    if manifest.get('workspace', {}).get('package', {}).get('version') != version:
        raise ValueError(f'Tag {tag} must match Cargo.toml [workspace.package].version; update and commit version before tagging')
    with (root / 'Cargo.lock').open('rb') as file:
        lock = tomllib.load(file)
    for name in ('yufu-root', 'yufu', 'auth'):
        packages = [p for p in lock.get('package', []) if p['name'] == name and 'source' not in p]
        if len(packages) != 1 or packages[0]['version'] != version:
            raise ValueError(f'Cargo.lock workspace package {name} must match {version}; refresh and commit the lockfile')
    return version, prerelease


def basename(tag, target):
    parse_tag(tag)
    if target not in TARGETS:
        raise ValueError(f'Unsupported target: {target}')
    return f'yufu-{tag}-{target}'


def archive_name(tag, target):
    return f'{basename(tag, target)}.{TARGETS[target]}'


def binary_version(binary):
    result = subprocess.run([str(binary.resolve()), '--version'], check=True, capture_output=True, text=True, timeout=30)
    output = result.stdout.strip()
    if not output.startswith('yufu '):
        raise ValueError(f'Unexpected --version output: {output!r}')
    return output.removeprefix('yufu ')


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda: file.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def payload_names(target):
    names = {'yufu.exe' if 'windows' in target else 'yufu',
             'yufu.example.toml', 'README.md', 'BUILD-INFO.json'}
    if 'linux' in target:
        names.add('yufu.service')
    return names


def require_webui(root):
    folder = root / 'assets/web'
    index = folder / 'index.html'
    if not index.is_file():
        raise ValueError('Missing WebUI index.html; run pnpm build:web before Cargo build')
    # Require the actual Vite JS/CSS references, not merely a leftover index.html.
    references = re.findall(r'(?:src|href)=["\'](/assets/[^"\']+\.(?:js|css)(?:\?[^"\']*)?)["\']', index.read_text())
    if not references or any(not (folder / ref.lstrip('/').split('?')[0]).is_file() for ref in references):
        raise ValueError('Missing referenced WebUI JS/CSS assets')


def package(root, tag, target, binary, output, commit):
    version, _ = validate(root, tag)
    base = basename(tag, target)
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('Expected full Git commit SHA')
    if not binary.is_file() or binary.is_symlink():
        raise FileNotFoundError(f'Missing regular binary: {binary}')
    require_webui(root)
    if binary_version(binary) != version:
        raise ValueError(f'Binary version must match tag {tag}')
    output.mkdir(parents=True, exist_ok=True)
    destination = output / archive_name(tag, target)
    info = {
        'tag': tag, 'version': version, 'commit': commit, 'target': target,
        'environment': {
            'os': platform.platform(), 'python': platform.python_version(),
            'rust': subprocess.run(['rustc', '--version'], check=True, capture_output=True, text=True).stdout.strip(),
            'node': os.environ.get('RELEASE_NODE_VERSION', 'not recorded'),
            'pnpm': os.environ.get('RELEASE_PNPM_VERSION', 'not recorded'),
        },
    }
    with tempfile.TemporaryDirectory() as temporary:
        stage = Path(temporary) / base
        stage.mkdir()
        executable = 'yufu.exe' if 'windows' in target else 'yufu'
        sources = {executable: binary, 'yufu.example.toml': root / 'conf/yufu.example.toml',
                   'README.md': root / 'docs/releases/installation.md'}
        if 'linux' in target:
            sources['yufu.service'] = root / 'conf/yufu.service'
        for name, source in sources.items():
            if source.is_symlink() or not source.is_file():
                raise ValueError(f'Missing regular package input: {source}')
            shutil.copyfile(source, stage / name)
            (stage / name).chmod(0o755 if name == executable else 0o644)
        (stage / 'BUILD-INFO.json').write_text(json.dumps(info, indent=2) + '\n', encoding='utf-8')
        if TARGETS[target] == 'zip':
            with zipfile.ZipFile(destination, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
                for source in sorted(stage.iterdir()):
                    archive.write(source, f'{base}/{source.name}')
        else:
            with tarfile.open(destination, 'w:gz') as archive:
                for source in sorted(stage.iterdir()):
                    archive.add(source, arcname=f'{base}/{source.name}', recursive=False)
    inspect_archive(destination, tag, target, commit)
    return destination


def inspect_archive(path, tag, target, commit=None):
    base = basename(tag, target)
    if path.name != archive_name(tag, target) or path.is_symlink():
        raise ValueError(f'Unexpected archive name/type: {path.name}')
    expected = {f'{base}/{name}' for name in payload_names(target)}
    entries = []
    metadata = None
    executable = f'{base}/yufu' + ('.exe' if 'windows' in target else '')

    def inspect(name, regular, mode, size, read):
        nonlocal metadata
        if name not in expected or not regular or PurePosixPath(name).is_absolute() or '..' in PurePosixPath(name).parts:
            raise ValueError(f'Unsafe or unexpected archive member: {name}')
        if size <= 0:
            raise ValueError(f'Empty package input: {name}')
        if name == executable and 'windows' not in target and mode & 0o111 != 0o111:
            raise ValueError('Unix executable permission missing')
        entries.append(name)
        if name.endswith('/BUILD-INFO.json'):
            if size > 65536:
                raise ValueError('Oversized BUILD-INFO.json')
            metadata = json.loads(read())

    if TARGETS[target] == 'zip':
        with zipfile.ZipFile(path) as archive:
            for member in archive.infolist():
                mode = member.external_attr >> 16
                regular = not member.is_dir() and (mode & 0o170000) in (0, 0o100000)
                inspect(member.filename, regular, mode, member.file_size, lambda m=member: archive.read(m))
            if archive.testzip() is not None:
                raise ValueError('Corrupt ZIP contents')
    else:
        with tarfile.open(path, 'r:gz') as archive:
            for member in archive.getmembers():
                inspect(member.name, member.isfile(), member.mode, member.size,
                        lambda m=member: archive.extractfile(m).read())
    if set(entries) != expected or len(entries) != len(expected):
        raise ValueError('Missing or duplicate archive members')
    if not isinstance(metadata, dict) or any(metadata.get(key) != value for key, value in
           {'tag': tag, 'version': tag[1:], 'target': target}.items()):
        raise ValueError('Build metadata does not match archive')
    if not re.fullmatch(r'[0-9a-f]{40}', str(metadata.get('commit', ''))) or (commit and metadata['commit'] != commit):
        raise ValueError('Build commit does not match release')
    if not isinstance(metadata.get('environment'), dict) or not metadata['environment'].get('rust'):
        raise ValueError('Build environment missing')
    return metadata


def checksums(folder, tag, commit=None):
    expected = {archive_name(tag, target): target for target in TARGETS}
    found = {p.name for p in folder.iterdir()}
    if found - {'SHA256SUMS'} != set(expected):
        raise ValueError(f'Expected exactly four release archives; found {sorted(found)}')
    commits = set()
    for name, target in expected.items():
        commits.add(inspect_archive(folder / name, tag, target, commit)['commit'])
    if len(commits) != 1:
        raise ValueError('Archives were built from different commits')
    manifest = folder / 'SHA256SUMS'
    if manifest.is_symlink():
        raise ValueError('SHA256SUMS must not be a symlink')
    manifest.write_text(''.join(f'{sha256(folder / name)}  {name}\n' for name in sorted(expected)), encoding='utf-8')
    return manifest


def marker(commit):
    return f'<!-- yufu-release:{commit} -->'


class GitHub:
    """Use the job's GH_TOKEN through gh; fail closed on every error except an explicit lookup 404."""
    def __init__(self, repo):
        self.repo = repo

    def request(self, method, endpoint, data=None, allow_not_found=False):
        command = ['gh', 'api', '--method', method, endpoint]
        if data is not None:
            command += ['--input', '-']
        result = subprocess.run(command, input=json.dumps(data) if data is not None else None,
                                capture_output=True, text=True, timeout=120)
        if result.returncode:
            if allow_not_found and re.search(r'\(HTTP 404\)', result.stderr):
                return None
            # Do not print API responses/headers that could contain sensitive data.
            raise RuntimeError(f'GitHub API {method} {endpoint} failed (exit {result.returncode})')
        return json.loads(result.stdout) if result.stdout.strip() else None

    def upload(self, tag, paths):
        subprocess.run(['gh', 'release', 'upload', tag, *map(str, paths), '--repo', self.repo], check=True, timeout=1800)

    def download(self, tag, destination):
        subprocess.run(['gh', 'release', 'download', tag, '--repo', self.repo, '--dir', str(destination)], check=True, timeout=1800)


def publish(root, tag, folder, repo, commit, api=None):
    version, prerelease = validate(root, tag)
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repo):
        raise ValueError('Expected owner/repository')
    checksums(folder, tag, commit)
    api = api or GitHub(repo)
    prefix = f'repos/{repo}'

    def ensure_tag():
        obj = api.request('GET', f'{prefix}/git/ref/tags/{quote(tag, safe="")}')['object']
        for _ in range(8):
            if obj['type'] == 'commit':
                break
            if obj['type'] != 'tag' or not re.fullmatch(r'[0-9a-f]{40}', obj['sha']):
                raise ValueError('Invalid remote tag object')
            obj = api.request('GET', f'{prefix}/git/tags/{obj["sha"]}')['object']
        if obj['type'] != 'commit' or obj['sha'] != commit:
            raise ValueError('Remote tag moved; refusing to publish a different commit')

    ensure_tag()
    current = api.request('GET', f'{prefix}/releases/tags/{tag}', allow_not_found=True)
    if current is None:
        # The by-tag API documents published releases. Authenticated listing also
        # includes drafts, which is essential to resume an interrupted upload.
        matches = []
        page = 1
        while True:
            releases = api.request('GET', f'{prefix}/releases?per_page=100&page={page}')
            matches.extend(item for item in releases if item.get('tag_name') == tag)
            if len(releases) < 100:
                break
            page += 1
        if len(matches) > 1:
            raise ValueError('Multiple releases share this tag; manual review required')
        current = matches[0] if matches else None
    if current and (not current.get('draft') or current.get('tag_name') != tag
                    or marker(commit) not in (current.get('body') or '')):
        raise ValueError('Release is already public or draft is not owned by this workflow/commit')
    notes = api.request('POST', f'{prefix}/releases/generate-notes', {'tag_name': tag, 'target_commitish': commit})
    body = (f'{marker(commit)}\n\n## yufu {version}\n\n'
            f'Source tag: `{tag}`\n\nCommit: `{commit}`\n\n'
            'Packages include the embedded WebUI, configuration example and installation instructions.\n'
            'Platforms: Linux x86_64 GNU/glibc, Windows x86_64 MSVC, macOS x86_64 and ARM64.\n'
            'Packages are not code-signed or notarized. Linux is built on Ubuntu 22.04.\n\n'
            'Download the matching package and SHA256SUMS. On Linux use `sha256sum --check --ignore-missing SHA256SUMS`; '
            'on macOS use `shasum -a 256 --check --ignore-missing SHA256SUMS`. '
            'On Windows use `Get-FileHash -Algorithm SHA256 <archive>` and compare its entry.\n\n'
            '[Installation guide](https://github.com/' + repo + '/blob/' + tag + '/docs/releases/installation.md)\n\n'
            + notes.get('body', ''))
    data = {'tag_name': tag, 'target_commitish': commit, 'name': f'yufu {tag}',
            'body': body, 'draft': True, 'prerelease': prerelease}
    if current:
        release_id = current['id']
        api.request('PATCH', f'{prefix}/releases/{release_id}', data)
    else:
        release_id = api.request('POST', f'{prefix}/releases', data)['id']
    endpoint = f'{prefix}/releases/{release_id}'
    assets = api.request('GET', endpoint + '/assets?per_page=100')
    # Query uses a page size above our five expected files. Refuse suspiciously large drafts.
    if len(assets) >= 100:
        raise ValueError('Unexpected number of draft assets; manual review required')
    for asset in assets:
        api.request('DELETE', f'{prefix}/releases/assets/{asset["id"]}')
    paths = sorted(folder.iterdir())
    api.upload(tag, paths)
    assets = api.request('GET', endpoint + '/assets?per_page=100')
    expected = {path.name: path.stat().st_size for path in paths}
    if len(assets) != len(expected) or {a['name']: a['size'] for a in assets} != expected:
        raise ValueError('Uploaded asset set or sizes do not match local packages')
    with tempfile.TemporaryDirectory() as temporary:
        downloaded = Path(temporary)
        api.download(tag, downloaded)
        if {p.name for p in downloaded.iterdir()} != set(expected):
            raise ValueError('Downloaded asset set does not match')
        for source in paths:
            if sha256(source) != sha256(downloaded / source.name):
                raise ValueError(f'Downloaded checksum mismatch: {source.name}')
    ensure_tag()
    final = api.request('GET', endpoint)
    if not final or final['id'] != release_id or final.get('tag_name') != tag or not final.get('draft') or marker(commit) not in (final.get('body') or ''):
        raise ValueError('Release state changed during upload; refusing to publish')
    api.request('PATCH', endpoint, {'draft': False, 'prerelease': prerelease, 'make_latest': 'false' if prerelease else 'true'})
    print(f'Published {repo} {tag} from {commit}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for command in ('validate', 'package', 'checksums', 'publish'):
        sub = commands.add_parser(command)
        sub.add_argument('--tag', required=True)
        sub.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
        if command in ('package', 'checksums', 'publish'):
            sub.add_argument('--output', type=Path, required=True)
            sub.add_argument('--commit', required=True)
        if command == 'package':
            sub.add_argument('--target', choices=TARGETS, required=True)
            sub.add_argument('--binary', type=Path, required=True)
        if command == 'publish':
            sub.add_argument('--repo', required=True)
    args = parser.parse_args()
    if args.command == 'validate':
        version, prerelease = validate(args.root, args.tag)
        output = f'version={version}\nprerelease={str(prerelease).lower()}\n'
        print(output, end='')
        if os.environ.get('GITHUB_OUTPUT'):
            with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as file:
                file.write(output)
    elif args.command == 'package':
        print(package(args.root, args.tag, args.target, args.binary, args.output, args.commit))
    elif args.command == 'checksums':
        print(checksums(args.output, args.tag, args.commit))
    else:
        publish(args.root, args.tag, args.output, args.repo, args.commit)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, RuntimeError, OSError, KeyError, subprocess.SubprocessError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f'Release failed: {error}', file=sys.stderr)
        sys.exit(1)
