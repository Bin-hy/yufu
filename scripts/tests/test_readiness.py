"""Readiness evidence contracts; synthetic fixtures and disposable repositories only."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location('readiness', Path(__file__).parents[1] / 'readiness.py')
readiness = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(readiness)


class ReadinessTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nversion="0.1.0"\n')
        self.write('yufu/src/lib.rs', '// fixture\n')
        subprocess.run(['git', '-C', str(self.root), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(self.root), '-c', 'user.name=Fixture',
                        '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture'], check=True)

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)
        return path

    def test_baseline_tracks_changes_deletions_and_untracked_code(self):
        before = readiness.baseline(self.root)
        self.write('yufu/src/new.rs', '// new\n')
        self.assertNotEqual(before['digest'], readiness.baseline(self.root)['digest'])
        (self.root / 'yufu/src/new.rs').unlink()
        self.assertEqual(before['digest'], readiness.baseline(self.root)['digest'])
        (self.root / 'yufu/src/lib.rs').unlink()
        self.assertNotEqual(before['digest'], readiness.baseline(self.root)['digest'])

    def test_baseline_excludes_secrets_artifacts_and_symlinks(self):
        before = readiness.baseline(self.root)
        self.write('conf/yufu.local.toml', 'password="DO_NOT_READ"')
        self.write('conf/private.pem', 'DO_NOT_READ')
        self.write('docs/specs/product-readiness/evidence/runs/a/output.log', 'generated')
        self.write('assets/web/a.js', 'generated')
        self.write('yufu/src/.env', 'DO_NOT_READ')
        (self.root / 'yufu/src/link.rs').symlink_to(self.root / 'conf/yufu.local.toml')
        actual = readiness.baseline(self.root)
        self.assertEqual(before['digest'], actual['digest'])
        self.assertNotIn('DO_NOT_READ', json.dumps(actual))

    def record(self, output='test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'):
        folder = self.root / 'docs/specs/product-readiness/evidence/runs/fixture'
        folder.mkdir(parents=True)
        return readiness.write_record(self.root, folder, 'unit', readiness.CHECKS['unit'][0], 'test',
                                      output, 0, '2026-11-13T00:00:00+00:00',
                                      '2026-11-13T00:00:01+00:00')

    def test_metadata_requires_identity_time_and_test_counts(self):
        path = self.record()
        data = json.loads(path.read_text())
        self.assertEqual(data['test_count'], 3)
        for field in ('source', 'started_at', 'test_count', 'argv', 'environment'):
            broken = dict(data)
            del broken[field]
            path.write_text(json.dumps(broken))
            with self.assertRaises(ValueError):
                readiness.verify_record(self.root, path)
        path.write_text(json.dumps(data))
        self.assertEqual(readiness.verify_record(self.root, path)['status'], 'pass')

    def test_execution_fail_zero_or_missing_engine_never_passes(self):
        for output, code, kind, engines, status in [
            ('test result: ok. 0 passed; 0 failed;', 0, 'test', [], 'fail'),
            ('test result: ok. 2 passed; 0 failed;', 1, 'test', [], 'fail'),
            ('2 passed (1s)', 0, 'browser', ['chromium'], 'fail'),
            ('2 passed (1s)', 0, 'browser', ['chromium', 'firefox', 'webkit'], 'fail'),
            ('✓ 1 [chromium] case\n✓ 2 [firefox] case\n✓ 3 [webkit] case\n3 passed (1s)', 0, 'browser', ['chromium', 'firefox', 'webkit'], 'pass'),
            ('not available', None, 'check', [], 'blocked'),
        ]:
            with self.subTest(output=output, code=code, kind=kind):
                self.assertEqual(readiness.classify(output, code, kind, engines)[0], status)

    def test_execution_only_allows_named_checks(self):
        with self.assertRaises(ValueError):
            readiness.run_check(self.root, 'arbitrary-shell', self.root / 'out')

    def test_colourised_summaries_are_still_counted(self):
        # CI enables colour and Vitest wraps its summary in escape sequences. A
        # passing run must never be recorded as zero tests because of formatting.
        vitest = ('\x1b[2m      Tests \x1b[22m \x1b[1m\x1b[32m180 passed\x1b[39m'
                  '\x1b[22m\x1b[90m (180)\x1b[39m')
        self.assertEqual(readiness.test_count(vitest), 180)
        self.assertEqual(readiness.classify(vitest, 0, 'test'), ('pass', 180))
        self.assertEqual(
            readiness.classify('test result: \x1b[32mok\x1b[0m. 7 passed; 0 failed;', 0, 'test'),
            ('pass', 7))

    def test_launch_keeps_the_canonical_command(self):
        # Everywhere but Windows the recorded command is launched unchanged.
        with mock.patch.object(readiness.os, 'name', 'posix'):
            self.assertEqual(readiness.launch(['pnpm', 'build:web']), ['pnpm', 'build:web'])

    def test_launch_wraps_windows_batch_shims(self):
        # npm exposes pnpm as a .cmd batch file, which CreateProcess cannot start.
        with mock.patch.object(readiness.os, 'name', 'nt'), \
                mock.patch.object(readiness.shutil, 'which', return_value=r'C:\npm\pnpm.cmd'):
            command = readiness.launch(['pnpm', 'build:web'])
        self.assertTrue(command[0].lower().endswith('cmd.exe'))
        self.assertEqual(command[1:], ['/c', r'C:\npm\pnpm.cmd', 'build:web'])

    def test_launch_leaves_an_unresolved_program_for_subprocess(self):
        # Returning argv unchanged keeps a missing tool a blocked record rather
        # than a failed check, because subprocess.run raises FileNotFoundError.
        with mock.patch.object(readiness.os, 'name', 'nt'), \
                mock.patch.object(readiness.shutil, 'which', return_value=None):
            self.assertEqual(readiness.launch(['pnpm', 'build:web']), ['pnpm', 'build:web'])

    def test_execution_redacts_known_secret_formats(self):
        secret = 'FICTIONAL_SECRET_SENTINEL_123456'
        for value in [f'Authorization: Bearer {secret}', f'Cookie: sso={secret}',
                      f'https://user:{secret}@db.invalid', f'#kind=invitation&secret={secret}',
                      f'{{"client_secret":"{secret}"}}',
                      f'-----BEGIN PRIVATE KEY-----\n{secret}\n-----END PRIVATE KEY-----']:
            with self.subTest(value=value[:25]):
                self.assertNotIn(secret, readiness.redact(value))

    def test_integrity_detects_tampering_old_source_and_traversal(self):
        path = self.record()
        original = path.read_text()
        data = json.loads(original)
        output = path.parent / 'output.log'
        contents = output.read_text()
        output.write_text('tampered')
        with self.assertRaises(ValueError): readiness.verify_record(self.root, path)
        output.write_text(contents)
        data['output']['path'] = '../outside.log'
        path.write_text(json.dumps(data))
        with self.assertRaises(ValueError): readiness.verify_record(self.root, path)
        path.write_text(original)
        self.write('yufu/src/lib.rs', '// changed\n')
        with self.assertRaises(ValueError): readiness.verify_record(self.root, path)

    def capability(self):
        self.write('README.md', 'fixture documentation')
        return {'id': 'oidc', 'scope': 'public', 'status': 'implemented_pending_validation',
                'version': '0.1.0', 'acceptance': ['AC3'], 'evidence': [],
                'limitations': ['Synthetic fixture'], 'docs': ['README.md']}

    def test_capabilities_reject_invalid_or_unverified_claims(self):
        item = self.capability()
        readiness.validate_capabilities(self.root, {'schema_version': 1, 'capabilities': [item]})
        for field, value in [('status', 'ready'), ('scope', 'anything'),
                             ('acceptance', ['AC99']), ('docs', ['../outside']),
                             ('status', 'verified_available')]:
            bad = dict(item, **{field: value})
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                readiness.validate_capabilities(self.root, {'schema_version': 1, 'capabilities': [bad]})
        with self.assertRaises(ValueError):
            readiness.validate_capabilities(self.root, {'schema_version': 1, 'capabilities': [item, item]})

    def test_integrity_cannot_turn_failed_record_into_pass(self):
        path = self.record('test result: ok. 0 passed; 0 failed;')
        data = json.loads(path.read_text())
        data['status'] = 'pass'
        data['test_count'] = 99
        path.write_text(json.dumps(data))
        with self.assertRaises(ValueError): readiness.verify_record(self.root, path)

    def test_metadata_rejects_relabelled_commands(self):
        path = self.record()
        original = json.loads(path.read_text())
        for changes in ({'check': 'unknown'}, {'argv': ['echo', 'pass']}, {'kind': 'check'}):
            path.write_text(json.dumps(dict(original, **changes)))
            with self.assertRaises(ValueError): readiness.verify_record(self.root, path)

    def test_metadata_rejects_malformed_nested_fields(self):
        path = self.record()
        original = json.loads(path.read_text())
        for field, value in [('check', []), ('environment', []), ('source', None),
                             ('engines', [[]]), ('argv', [42]), ('output', 'bad'),
                             ('exit_code', False), ('test_count', True)]:
            path.write_text(json.dumps(dict(original, **{field: value})))
            with self.subTest(field=field), self.assertRaises(ValueError):
                readiness.verify_record(self.root, path)


if __name__ == '__main__':
    unittest.main()
