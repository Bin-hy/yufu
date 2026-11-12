"""Release contracts use temporary repositories and an in-memory GitHub CLI substitute."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SPEC = importlib.util.spec_from_file_location('release', Path(__file__).parents[1] / 'release.py')
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
SHA = 'a' * 40


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.version('1.2.3')
        for name, content in {
            'conf/yufu.example.toml': '[http]\nlisten="127.0.0.1:4000"',
            'conf/yufu.service': '[Service]\nExecStart=/usr/bin/yufu',
            'docs/releases/installation.md': '# Install',
            'assets/web/index.html': '<script src="/assets/app.js"></script>',
            'assets/web/assets/app.js': 'console.log("demo")',
            'conf/private.local.pem': 'FICTIONAL_PRIVATE_KEY',
            'node_modules/forbidden': 'not for distribution',
        }.items():
            p = self.root / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content)
        self.binary = self.root / 'binary'
        self.binary.write_bytes(b'fake executable')
        self.out = self.root / 'out'

    def version(self, value, locked=None):
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nversion="' + value + '"\n')
        (self.root / 'Cargo.lock').write_text('\n'.join(
            '[[package]]\nname="' + name + '"\nversion="' + (locked or value) + '"\n'
            for name in ('yufu-root', 'yufu', 'auth')))

    def package(self, target, tag='v1.2.3'):
        with patch.object(release, 'binary_version', return_value=tag[1:]):
            return release.package(self.root, tag, target, self.binary, self.out, SHA)

    def all_packages(self, tag='v1.2.3'):
        self.version(tag[1:])
        return [self.package(target, tag) for target in release.TARGETS]

    def test_versions(self):
        for tag in ('v0.0.0', 'v1.2.3', 'v1.2.3-alpha.0', 'v1.2.3-beta.2', 'v1.2.3-rc.3'):
            with self.subTest(tag=tag):
                self.version(tag[1:])
                self.assertEqual(release.validate(self.root, tag), (tag[1:], '-' in tag))

    def test_invalid_versions(self):
        for tag in ('1.2.3', 'v1.2', 'v01.2.3', 'v1.02.3', 'v1.2.03', 'v1.2.3-rc',
                    'v1.2.3-rc.01', 'v1.2.3-preview.1', 'v1.2.3+meta', 'v1.2.3; touch x', 'v1.2.3\n'):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate(self.root, tag)

    def test_version_sources_must_match(self):
        for version, locked in [('1.2.4', '1.2.4'), ('1.2.3', '1.2.4')]:
            self.version(version, locked)
            with self.assertRaises(ValueError):
                release.validate(self.root, 'v1.2.3')
        for filename in ('Cargo.toml', 'Cargo.lock'):
            self.version('1.2.3')
            (self.root / filename).unlink()
            with self.assertRaises((ValueError, FileNotFoundError)):
                release.validate(self.root, 'v1.2.3')

    def test_all_archives_have_exact_whitelist(self):
        for target in release.TARGETS:
            with self.subTest(target=target):
                path = self.package(target)
                info = release.inspect_archive(path, 'v1.2.3', target, SHA)
                self.assertEqual(info['commit'], SHA)
                self.assertEqual(info['target'], target)
                self.assertNotIn(b'FICTIONAL_PRIVATE_KEY', path.read_bytes())
                self.assertTrue(path.name.endswith('.zip' if 'windows' in target else '.tar.gz'))

    def test_packaging_preconditions(self):
        with self.assertRaises(ValueError):
            self.package('unsupported')
        with patch.object(release, 'binary_version', return_value='9.9.9'), self.assertRaises(ValueError):
            release.package(self.root, 'v1.2.3', next(iter(release.TARGETS)), self.binary, self.out, SHA)
        self.binary.unlink()
        with self.assertRaises(FileNotFoundError):
            self.package(next(iter(release.TARGETS)))

    def test_webui_required(self):
        for filename in ('index.html', 'assets/app.js'):
            path = self.root / 'assets/web' / filename
            data = path.read_bytes()
            path.unlink()
            with self.assertRaises(ValueError):
                self.package(next(iter(release.TARGETS)))
            path.write_bytes(data)

    def test_checksums_and_tampering(self):
        archives = self.all_packages()
        manifest = release.checksums(self.out, 'v1.2.3', SHA)
        original = manifest.read_text()
        self.assertEqual(original, release.checksums(self.out, 'v1.2.3', SHA).read_text())
        for line in original.splitlines():
            digest, name = line.split('  ')
            self.assertEqual(digest, hashlib.sha256((self.out / name).read_bytes()).hexdigest())
        archives[0].write_bytes(b'broken archive')
        with self.assertRaises((ValueError, tarfile.TarError, zipfile.BadZipFile)):
            release.checksums(self.out, 'v1.2.3', SHA)

    def test_missing_and_extra_assets(self):
        archives = self.all_packages()
        extra = self.out / 'private.pem'
        extra.write_text('fictional secret')
        with self.assertRaises(ValueError):
            release.checksums(self.out, 'v1.2.3', SHA)
        extra.unlink()
        archives[0].unlink()
        with self.assertRaises(ValueError):
            release.checksums(self.out, 'v1.2.3', SHA)

    def test_unsafe_or_incomplete_archive(self):
        target = next(iter(release.TARGETS))
        path = self.package(target)
        for name, kind in [('../escape', tarfile.REGTYPE), ('wrong/file', tarfile.REGTYPE),
                           ('link', tarfile.SYMTYPE)]:
            with self.subTest(name=name):
                with tarfile.open(path, 'w:gz') as archive:
                    entry = tarfile.TarInfo(name)
                    entry.type = kind
                    archive.addfile(entry, io.BytesIO())
                with self.assertRaises(ValueError):
                    release.inspect_archive(path, 'v1.2.3', target, SHA)

    def test_metadata_and_executable_mode(self):
        target = next(iter(release.TARGETS))
        for mutate in ('commit', 'target', 'permission'):
            path = self.package(target)
            with tarfile.open(path) as archive:
                members = [(m, archive.extractfile(m).read()) for m in archive.getmembers()]
            with tarfile.open(path, 'w:gz') as archive:
                for member, data in members:
                    if member.name.endswith('BUILD-INFO.json') and mutate != 'permission':
                        info = json.loads(data)
                        info[mutate] = 'bad'
                        data = json.dumps(info).encode()
                    if member.name.endswith('/yufu') and mutate == 'permission':
                        member.mode = 0o644
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                release.inspect_archive(path, 'v1.2.3', target, SHA)

    def test_api_only_explicit_lookup_404_is_absent(self):
        api = release.GitHub('owner/repo')
        for status in (401, 403, 404, 500):
            response = subprocess.CompletedProcess([], 1, '', f'gh: failed (HTTP {status})')
            with self.subTest(status=status), patch.object(release.subprocess, 'run', return_value=response):
                if status == 404:
                    self.assertIsNone(api.request('GET', 'repos/owner/repo/releases/tags/v1.2.3', allow_not_found=True))
                else:
                    with self.assertRaises(RuntimeError):
                        api.request('GET', 'repos/owner/repo/releases/tags/v1.2.3', allow_not_found=True)
                with self.assertRaises(RuntimeError):
                    api.request('GET', 'repos/owner/repo/releases/tags/v1.2.3')

    def test_draft_with_null_body_is_rejected(self):
        self.all_packages()
        api = FakeGitHub(self.out, failure='foreign')
        api.current['body'] = None
        with self.assertRaises(ValueError):
            release.publish(self.root, 'v1.2.3', self.out, 'owner/repo', SHA, api)
        self.assertEqual(api.mutations, [])

    def test_late_tag_movement_leaves_draft(self):
        self.all_packages()
        api = FakeGitHub(self.out)
        original = api.download
        def moved_after_download(tag, destination):
            original(tag, destination)
            api.failure = 'moved'
        api.download = moved_after_download
        with self.assertRaises(ValueError):
            release.publish(self.root, 'v1.2.3', self.out, 'owner/repo', SHA, api)
        self.assertTrue(api.current['draft'])
        self.assertEqual(api.published, 0)

    def test_zip_rejects_links_and_extras(self):
        target = 'x86_64-pc-windows-msvc'
        for kind in ('link', 'duplicate'):
            path = self.package(target)
            if kind == 'link':
                with zipfile.ZipFile(path, 'w') as archive:
                    member = zipfile.ZipInfo(release.basename('v1.2.3', target) + '/yufu.exe')
                    member.create_system = 3
                    member.external_attr = 0o120777 << 16
                    archive.writestr(member, b'/tmp/link')
            else:
                with zipfile.ZipFile(path, 'a') as archive:
                    archive.writestr('unexpected', b'extra')
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                release.inspect_archive(path, 'v1.2.3', target, SHA)

    def test_publish_success_and_public_rerun(self):
        self.all_packages()
        api = FakeGitHub(self.out)
        release.publish(self.root, 'v1.2.3', self.out, 'owner/repo', SHA, api)
        self.assertFalse(api.current['draft'])
        self.assertFalse(api.current['prerelease'])
        self.assertEqual(len(api.assets), 5)
        self.assertIn('Generated changes', api.current['body'])
        mutations = len(api.mutations)
        with self.assertRaises(ValueError):
            release.publish(self.root, 'v1.2.3', self.out, 'owner/repo', SHA, api)
        self.assertEqual(len(api.mutations), mutations)

    def test_prerelease(self):
        self.all_packages('v1.2.3-rc.1')
        api = FakeGitHub(self.out)
        release.publish(self.root, 'v1.2.3-rc.1', self.out, 'owner/repo', SHA, api)
        self.assertTrue(api.current['prerelease'])

    def test_failures_do_not_publish_and_retry_works(self):
        for failure in ('upload', 'missing', 'size', 'digest'):
            with self.subTest(failure=failure):
                self.all_packages()
                api = FakeGitHub(self.out, failure=failure)
                with self.assertRaises((ValueError, RuntimeError)):
                    release.publish(self.root, 'v1.2.3', self.out, 'owner/repo', SHA, api)
                self.assertTrue(api.current['draft'])
                api.failure = None
                release.publish(self.root, 'v1.2.3', self.out, 'owner/repo', SHA, api)
                self.assertEqual(api.published, 1)

    def test_early_rejections_never_write(self):
        self.all_packages()
        for failure in ('moved', 'auth', 'foreign', 'other-commit', 'public'):
            with self.subTest(failure=failure):
                api = FakeGitHub(self.out, failure=failure)
                with self.assertRaises((ValueError, RuntimeError)):
                    release.publish(self.root, 'v1.2.3', self.out, 'owner/repo', SHA, api)
                self.assertEqual(api.mutations, [])


class FakeGitHub:
    def __init__(self, folder, failure=None):
        self.folder, self.failure = folder, failure
        self.current = None
        self.assets = []
        self.mutations = []
        self.published = 0
        if failure in ('foreign', 'other-commit', 'public'):
            self.current = {'id': 1, 'tag_name': 'v1.2.3', 'draft': failure != 'public',
                            'body': '' if failure == 'foreign' else release.marker('b' * 40)}

    def request(self, method, endpoint, data=None, allow_not_found=False):
        if method != 'GET':
            self.mutations.append((method, endpoint, data))
        if '/git/ref/tags/' in endpoint:
            return {'object': {'type': 'tag', 'sha': 'c' * 40}}
        if '/git/tags/' in endpoint:
            return {'object': {'type': 'commit', 'sha': 'b' * 40 if self.failure == 'moved' else SHA}}
        if self.failure == 'auth':
            raise RuntimeError('HTTP 403')
        if '/releases/tags/' in endpoint:
            return self.current if self.current and not self.current.get('draft') else None
        if '/releases?per_page=' in endpoint:
            return [self.current] if self.current else []
        if method == 'GET' and endpoint.endswith('/releases/1'):
            return self.current
        if endpoint.endswith('/generate-notes'):
            return {'body': 'Generated changes'}
        if endpoint.split('?')[0].endswith('/assets'):
            if self.failure == 'missing': return self.assets[:-1]
            if self.failure == 'size': return [dict(a, size=0) for a in self.assets]
            return self.assets
        if method == 'DELETE':
            self.assets = [a for a in self.assets if a['id'] != int(endpoint.split('/')[-1])]
            return None
        if method == 'POST' and endpoint.endswith('/releases'):
            self.current = dict(data, id=1)
            return self.current
        if method == 'PATCH':
            self.current.update(data)
            if data.get('draft') is False: self.published += 1
            return self.current
        raise AssertionError((method, endpoint))

    def upload(self, tag, paths):
        if self.failure == 'upload': raise RuntimeError('upload interrupted')
        self.assets = [{'id': i, 'name': p.name, 'size': p.stat().st_size} for i, p in enumerate(paths, 1)]

    def download(self, tag, destination):
        for asset in self.assets:
            shutil.copyfile(self.folder / asset['name'], destination / asset['name'])
        if self.failure == 'digest':
            (destination / self.assets[0]['name']).write_bytes(b'tampered')


if __name__ == '__main__':
    unittest.main()
