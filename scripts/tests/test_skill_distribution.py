"""The distributed skill must pin the same versions its scripts and docs claim."""
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SKILL = ROOT / '.agents' / 'skills' / 'yufu-onboarding'
VERSIONS = SKILL / 'assets' / 'versions.env'
SDK_MOD = ROOT / 'examples' / 'sdk' / 'go' / 'go.mod'
TARGETS = ('aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu')


def read_versions():
    values = {}
    for line in VERSIONS.read_text(encoding='utf-8').splitlines():
        line = line.strip()
        if not line or line.startswith('#') or '=' not in line:
            continue
        key, value = line.split('=', 1)
        values[key] = value
    return values


class SkillDistributionTests(unittest.TestCase):
    def test_sdk_version_matches_example_module(self):
        versions = read_versions()
        match = re.search(r'github\.com/Bin-hy/yufu-sdk-go\s+(v\S+)', SDK_MOD.read_text(encoding='utf-8'))
        self.assertIsNotNone(match, 'examples/sdk/go/go.mod does not require the SDK')
        self.assertEqual(versions['YUFU_SDK_VERSION'], match.group(1))

    def test_pinned_yufu_version_has_a_tag(self):
        if shutil.which('git') is None:
            self.skipTest('git is not available')
        version = read_versions()['YUFU_VERSION']
        result = subprocess.run(
            ['git', 'tag', '--list'],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        tags = result.stdout.splitlines()
        if version in tags:
            return
        shallow = subprocess.run(
            ['git', 'rev-parse', '--is-shallow-repository'],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        if shallow.stdout.strip() == 'true' and not tags:
            self.skipTest('shallow clones without fetched tags cannot verify the tag')
        self.fail(f'versions.env pins {version}, but no such tag exists')

    def test_checksums_cover_every_supported_target(self):
        versions = read_versions()
        for target in TARGETS:
            key = 'YUFU_SHA256_' + target.replace('-', '_')
            with self.subTest(target=target):
                self.assertIn(key, versions)
                self.assertRegex(versions[key], r'^[0-9a-f]{64}$')

    def test_skill_does_not_reference_monorepo_examples(self):
        offenders = []
        for path in SKILL.rglob('*'):
            if not path.is_file() or '__pycache__' in path.parts:
                continue
            try:
                text = path.read_text(encoding='utf-8')
            except UnicodeDecodeError:
                continue
            if 'examples/sdk' in text:
                offenders.append(str(path.relative_to(SKILL)))
        self.assertEqual(offenders, [])


if __name__ == '__main__':
    unittest.main()
