#!/usr/bin/env python3
"""Local, bounded readiness evidence. Standard library only; never publishes or deploys."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
import uuid

MAX_OUTPUT = 4 * 1024 * 1024
EVIDENCE = os.environ.get('YUFU_READINESS_OUTPUT', 'docs/specs/product-readiness/evidence/runs')
STATUSES = {'verified_available', 'implemented_pending_validation', 'legacy_compatibility', 'unsupported'}
CHECKS = {
    'tool-tests': ([sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests', '-v'], 'test', '.'),
    'unit': (['cargo', 'test', '-p', 'yufu', '--lib', '--', '--test-threads=1'], 'test', '.'),
    'auth-tests': (['cargo', 'test', '-p', 'auth'], 'test', '.'),
    'fmt': (['cargo', 'fmt', '--all', '--', '--check'], 'check', '.'),
    'clippy': (['cargo', 'clippy', '--locked', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings'], 'check', '.'),
    'rust-tests': (['cargo', 'test', '--locked', '--workspace', '--all-targets', '--all-features', '--', '--test-threads=1'], 'test', '.'),
    'rust-build': (['cargo', 'build', '--locked', '--workspace', '--all-targets', '--all-features', '--release', '--verbose'], 'check', '.'),
    'rust-doc': (['cargo', 'test', '--locked', '--workspace', '--doc', '--all-features'], 'test', '.'),
    'web-tests': (['pnpm', '--filter', '@yufu/web', 'test'], 'test', '.'),
    'web-check': (['pnpm', '--filter', '@yufu/web', 'check'], 'check', '.'),
    'web-lint': (['pnpm', '--filter', '@yufu/web', 'lint'], 'check', '.'),
    'web-build': (['pnpm', 'build:web'], 'check', '.'),
    'examples-check': (['pnpm', 'check:examples'], 'check', '.'),
    'examples-tests': (['pnpm', 'test:examples'], 'test', '.'),
    'website-check': (['pnpm', 'check:website'], 'check', '.'),
    'website-build': (['pnpm', 'build:website'], 'check', '.'),
    'machete': (['cargo', 'machete'], 'check', '.'),
    'audit': (['cargo', 'audit'], 'check', '.'),
    'browser': (['node', 'tests/e2e/enterprise-sso-runner.mjs'], 'browser', 'web'),
    'performance': (['cargo', 'test', '--release', '-p', 'yufu-root', '--test', 'mod',
                     'product_readiness_capacity', '--', '--ignored', '--nocapture', '--test-threads=1'], 'test', '.'),
    'upgrade': (['cargo', 'test', '--release', '-p', 'yufu-root', '--test', 'mod',
                 'upgrade_drill', '--', '--ignored', '--nocapture', '--test-threads=1'], 'test', '.'),
}


def utc_now():
    return datetime.now(timezone.utc).isoformat()


def environment_metadata():
    tools = {}
    for name in ('rustc', 'cargo', 'node', 'pnpm'):
        try:
            result = subprocess.run([name, '--version'], capture_output=True, timeout=5, check=False)
            tools[name] = redact(result.stdout[:1024].decode('utf-8', errors='replace').strip()) if result.returncode == 0 else None
        except (OSError, subprocess.TimeoutExpired):
            tools[name] = None
    return {'os': platform.system(), 'architecture': platform.machine(),
            'python': platform.python_version(), 'tools': tools,
            'ci': {key: os.environ[key] for key in ('GITHUB_REPOSITORY', 'GITHUB_SHA', 'GITHUB_RUN_ID',
                   'GITHUB_RUN_ATTEMPT', 'GITHUB_JOB') if key in os.environ}}


def digest_bytes(value):
    return hashlib.sha256(value).hexdigest()


def confined(root, relative):
    rel = PurePosixPath(relative)
    if rel.is_absolute() or '..' in rel.parts or not rel.parts or '\\' in relative:
        raise ValueError('Expected a confined relative path')
    path = root / str(rel)
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError('Path escapes repository')
    for candidate in (path, *path.parents):
        if candidate == root:
            break
        if candidate.is_symlink():
            raise ValueError('Symlinks are not evidence inputs')
    return path


def source_allowed(name):
    p = PurePosixPath(name)
    if any(part in {'node_modules', 'target', 'dist', '.git', '__pycache__', 'evidence'} for part in p.parts):
        return False
    if any(part.startswith('.') and part not in {'.github'} for part in p.parts):
        return False
    if '.local.' in p.name or p.suffix in {'.pem', '.key', '.p12', '.pfx'}:
        return False
    if p.parts[0] == 'conf':
        return p.name.endswith('.example.toml') or p.name == 'yufu.service'
    if p.parts[0] == 'deploy':
        return p.name in {'Dockerfile', 'Caddyfile', 'compose.yaml', 'README.md',
                          'env.example', 'yufu.toml.example', 'docker-entrypoint.sh'}
    if name in {'Cargo.toml', 'Cargo.lock', 'package.json', 'pnpm-lock.yaml',
                'pnpm-workspace.yaml', 'compose.yaml', 'README.md'}:
        return True
    roots = {'src', 'yufu', 'libs', 'web', 'website', 'sdk', 'examples', 'scripts', '.github'}
    extensions = {'.rs', '.py', '.ts', '.tsx', '.mjs', '.js', '.vue', '.css', '.html',
                  '.toml', '.json', '.yaml', '.yml', '.md', '.mdx', '.go', '.mod', '.sum'}
    return p.parts[0] in roots and p.suffix in extensions


def git(root, *args):
    return subprocess.run(['git', '-C', str(root), *args], capture_output=True,
                          check=True, timeout=30).stdout


def baseline(root):
    root = root.resolve()
    names = set(git(root, 'ls-files', '-z', '--cached', '--others', '--exclude-standard').decode().split('\0'))
    files = {}
    for name in sorted(names):
        if not name or not source_allowed(name):
            continue
        try:
            path = confined(root, name)
        except ValueError:
            continue
        if not path.exists():
            files[name] = 'deleted'
        elif path.is_file():
            if path.stat().st_size > 16 * 1024 * 1024:
                raise ValueError('Oversized source input: ' + name)
            files[name] = digest_bytes(path.read_bytes())
    return {'head': git(root, 'rev-parse', 'HEAD').decode().strip(),
            'dirty': bool(git(root, 'status', '--porcelain', '--untracked-files=normal')),
            'digest': digest_bytes(json.dumps(files, sort_keys=True).encode()), 'files': files}


def redact(output):
    output = re.sub(r'-----BEGIN [^-]*(?:PRIVATE KEY|CERTIFICATE)-----.*?(?:-----END [^-]+-----|\Z)',
                    '[REDACTED PEM]', output, flags=re.S)
    output = re.sub(r'(?im)\b(?:authorization|proxy-authorization|cookie|set-cookie)\s*[:=][^\r\n]*',
                    '[REDACTED HEADER]', output)
    output = re.sub(r'(?i)(https?://)[^\s/@]+:[^\s/@]+@', r'\1[REDACTED]@', output)
    output = re.sub(r'(?i)(["\']?(?:client_secret|access_token|refresh_token|id_token|password|secret|csrf_token|proof)["\']?\s*[:=]\s*["\']?)[^\s"\'&,}]+',
                    r'\1[REDACTED]', output)
    output = re.sub(r'\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+', '[REDACTED JWT]', output)
    return output


# Test runners colourise their summaries whenever colour is enabled, and CI sets
# CARGO_TERM_COLOR plus colour-capable reporters, so every pattern below would
# otherwise see escape sequences instead of the line it is counting. A passing
# run must never be recorded as zero tests because of terminal formatting.
ANSI_ESCAPE = re.compile(r'\x1b\[[0-9;]*[A-Za-z]')


def strip_ansi(text):
    return ANSI_ESCAPE.sub('', text)


def test_count(output):
    output = strip_ansi(output)
    rust = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;', output)
    python = re.findall(r'Ran (\d+) tests? in ', output)
    # Both Vitest and Playwright print summaries; ignore individual test title lines.
    js = re.findall(r'^\s*(?:Tests\s+)?(\d+) passed(?:\s|\()', output, re.M)
    return sum(int(a) + int(b) for a, b in rust) + sum(map(int, python)) + sum(map(int, js))


def classify(output, exit_code, kind, engines=()):
    output = strip_ansi(output)
    count = test_count(output) if kind in {'test', 'browser'} else None
    if exit_code is None:
        return 'blocked', count
    if exit_code != 0 or (kind in {'test', 'browser'} and not count):
        return 'fail', count
    observed = set(re.findall(r'(?:✓|✔|ok)\s+\d*\s*\[(chromium|firefox|webkit)\]', output))
    if kind == 'browser' and (set(engines) != observed or observed != {'chromium', 'firefox', 'webkit'} or
                              re.search(r'\b[1-9]\d* skipped\b', output)):
        return 'fail', count
    return 'pass', count


def write_record(root, folder, check, argv, kind, output, exit_code, started_at, finished_at,
                 engines=(), source=None):
    folder.mkdir(parents=True, exist_ok=True)
    safe_output = redact(output)
    log = folder / 'output.log'
    log.write_text(safe_output, encoding='utf-8')
    status, count = classify(safe_output, exit_code, kind, engines)
    record = {'schema_version': 1, 'check': check, 'kind': kind, 'argv': argv,
              'source': source or baseline(root), 'started_at': started_at, 'finished_at': finished_at,
              'environment': environment_metadata(),
              'exit_code': exit_code, 'status': status, 'test_count': count,
              'engines': list(engines),
              'output': {'path': 'output.log', 'sha256': digest_bytes(log.read_bytes())}}
    path = folder / 'record.json'
    path.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return path


def verify_record(root, path):
    data = json.loads(path.read_text(encoding='utf-8'))
    required = {'schema_version', 'check', 'kind', 'argv', 'source', 'started_at', 'finished_at',
                'environment', 'exit_code', 'status', 'test_count', 'engines', 'output'}
    if not isinstance(data, dict) or not required <= data.keys() or data['schema_version'] != 1:
        raise ValueError('Missing or invalid evidence metadata')
    if (not isinstance(data['kind'], str) or data['kind'] not in {'check', 'test', 'browser'} or
            not isinstance(data['check'], str) or not isinstance(data['argv'], list) or not data['argv'] or
            not all(isinstance(arg, str) for arg in data['argv']) or
            not all(isinstance(data[key], dict) for key in ('environment', 'source', 'output')) or
            not isinstance(data['engines'], list) or not all(isinstance(engine, str) for engine in data['engines']) or
            (data['exit_code'] is not None and type(data['exit_code']) is not int) or
            (data['test_count'] is not None and type(data['test_count']) is not int)):
        raise ValueError('Invalid check kind or argv')
    expected = CHECKS.get(data['check'])
    if expected is None or data['kind'] != expected[1]:
        raise ValueError('Unknown or relabelled check')
    if data['check'] == 'tool-tests':
        valid_argv = (data['argv'][1:] == expected[0][1:] and
                      bool(re.fullmatch(r'python(?:\d+(?:\.\d+)?)?(?:\.exe)?', Path(data['argv'][0]).name)))
    else:
        valid_argv = data['argv'] == expected[0]
    if not valid_argv:
        raise ValueError('Evidence command does not match its named check')
    try:
        start, finish = [datetime.fromisoformat(data[key]) for key in ('started_at', 'finished_at')]
        if start.tzinfo is None or finish.tzinfo is None or finish < start:
            raise ValueError('Invalid evidence times')
        if not all(data['environment'].get(key) for key in ('os', 'architecture', 'python')):
            raise ValueError('Missing environment')
        current = baseline(root)
        if any(data['source'][key] != current[key] for key in ('head', 'digest', 'files')):
            raise ValueError('Evidence belongs to a different candidate source')
        log = confined(path.parent, data['output']['path'])
        if log.stat().st_size > MAX_OUTPUT or digest_bytes(log.read_bytes()) != data['output']['sha256']:
            raise ValueError('Output integrity mismatch')
        status, count = classify(log.read_text(encoding='utf-8'), data['exit_code'], data['kind'], data['engines'])
        if status != data['status'] or count != data['test_count']:
            raise ValueError('Evidence outcome does not match its output')
    except (KeyError, TypeError, OSError) as error:
        raise ValueError('Malformed evidence') from error
    return data


def validate_capabilities(root, data):
    if not isinstance(data, dict) or data.get('schema_version') != 1 or not data.get('capabilities'):
        raise ValueError('Invalid capability manifest')
    seen = set()
    for item in data['capabilities']:
        fields = {'id', 'scope', 'status', 'version', 'acceptance', 'evidence', 'limitations', 'docs'}
        if not isinstance(item, dict) or not fields <= item.keys():
            raise ValueError('Missing capability fields')
        if not isinstance(item['id'], str) or not re.fullmatch(r'[a-z][a-z0-9-]*', item['id']) or item['id'] in seen:
            raise ValueError('Invalid or duplicate capability id')
        seen.add(item['id'])
        if item['status'] not in STATUSES or item['scope'] not in {'public', 'internal', 'legacy'}:
            raise ValueError('Invalid capability state/scope')
        if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[a-z]+\.\d+)?', item['version']):
            raise ValueError('Missing capability version')
        if not item['acceptance'] or any(ac not in {f'AC{i}' for i in range(1, 17)} for ac in item['acceptance']):
            raise ValueError('Unknown acceptance criterion')
        if not isinstance(item['limitations'], list) or not isinstance(item['evidence'], list) or not item['docs']:
            raise ValueError('Invalid capability references')
        for doc in item['docs']:
            if not confined(root, doc).is_file():
                raise ValueError('Missing capability documentation')
        for evidence in item['evidence']:
            record = verify_record(root, confined(root, evidence))
            if record['status'] != 'pass':
                raise ValueError('Capability evidence has not passed')
        if item['status'] == 'verified_available' and not item['evidence']:
            raise ValueError('Verified availability requires candidate evidence')
    return data


def launch(argv):
    """Return the command to execute for an entry of CHECKS.

    npm installs pnpm as a .cmd shim on Windows. A batch file is not a Win32
    image, so CreateProcess cannot start it and the shim has to go through the
    command interpreter. Only the launched command changes: the recorded argv
    stays the canonical CHECKS entry that evidence verification compares against,
    and an unresolvable program is returned untouched so subprocess.run raises
    FileNotFoundError and the check records as blocked rather than failed.
    """
    if os.name != 'nt':
        return argv
    resolved = shutil.which(argv[0])
    if resolved and resolved.lower().endswith(('.cmd', '.bat')):
        return [os.environ.get('COMSPEC') or 'cmd.exe', '/c', resolved, *argv[1:]]
    return argv


def run_check(root, name, output_folder):
    if name not in CHECKS:
        raise ValueError('Unknown check; arbitrary commands are not allowed')
    argv, kind, cwd = CHECKS[name]
    source = baseline(root)
    started = utc_now()
    with tempfile.TemporaryFile() as capture:
        try:
            result = subprocess.run(launch(argv), cwd=root / cwd, stdout=capture,
                                    stderr=subprocess.STDOUT, timeout=7200, check=False)
            code = result.returncode
        except FileNotFoundError:
            capture.write(b'Check executable is unavailable.\n')
            code = None
        except subprocess.TimeoutExpired:
            capture.write(b'Check exceeded execution deadline.\n')
            code = 124
        capture.seek(0)
        raw = capture.read(MAX_OUTPUT + 1)
    if len(raw) > MAX_OUTPUT:
        raw = raw[:MAX_OUTPUT - 100] + b'\nOutput exceeded evidence limit; incomplete evidence rejected.\n'
        code = 125
    output = raw.decode('utf-8', errors='replace')
    # Engine names must come from actual successful case lines, not the command or a configured project list.
    engines = re.findall(r'(?:✓|✔|ok)\s+\d*\s*\[(chromium|firefox|webkit)\]', strip_ansi(output))
    if baseline(root)['digest'] != source['digest']:
        code = 126
        output += '\nCandidate source changed during execution; evidence rejected.\n'
    folder = output_folder / (time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()) + '-' + uuid.uuid4().hex[:8])
    return write_record(root, folder, name, argv, kind, output, code, started, utc_now(), engines, source)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    sub = parser.add_subparsers(dest='action', required=True)
    sub.add_parser('baseline')
    sub.add_parser('capabilities')
    run = sub.add_parser('run')
    run.add_argument('check', choices=sorted(CHECKS))
    verify = sub.add_parser('verify')
    verify.add_argument('--record', help='Repository-relative record.json')
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        if args.action == 'baseline':
            result = baseline(root)
        elif args.action == 'capabilities':
            data = json.loads((root / 'docs/product/capabilities.json').read_text(encoding='utf-8'))
            validate_capabilities(root, data)
            result = {'capabilities': len(data['capabilities']), 'valid': True}
        elif args.action == 'run':
            path = run_check(root, args.check, confined(root, EVIDENCE))
            record = verify_record(root, path)
            result = {'record': str(path.relative_to(root)), 'status': record['status'],
                      'test_count': record['test_count']}
        else:
            paths = [confined(root, args.record)] if args.record else sorted((confined(root, EVIDENCE)).glob('*/record.json'))
            if not paths:
                raise ValueError('No evidence records; nothing has been verified')
            records = [verify_record(root, path) for path in paths]
            result = {'records': len(records), 'status': 'pass' if all(r['status'] == 'pass' for r in records) else 'fail'}
        print(json.dumps(result, ensure_ascii=False, indent=2))
        if result.get('status') == 'blocked':
            return 2
        return 1 if result.get('status') == 'fail' else 0
    except (ValueError, OSError, subprocess.SubprocessError):
        print('Readiness evidence invalid or unavailable; check inputs and candidate identity.', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
