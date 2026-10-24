"""Mutation tests: ensure real contract drift cannot silently pass the guard."""
import copy
import importlib.util
from contextlib import contextmanager
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'api_policy', Path(__file__).with_name('check-api-policy.py'))
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class PolicyDriftTests(unittest.TestCase):
    def setUp(self):
        self.contract, self.workspace, self.packages = copy.deepcopy(policy.read_inputs())

    def errors(self, rust=None):
        msrv = self.contract['msrv']
        compiler = msrv + '.0' if len(msrv.split('.')) == 2 else msrv
        return policy.check(self.contract, self.workspace, self.packages, rust or compiler)

    def test_current_contract_and_ci_compilers(self):
        self.assertEqual(self.errors(), [])
        self.assertEqual(self.errors('stable'), [])

    def test_defaults_and_feature_forwarding_drift(self):
        for feature, value in [('default', ['web']), ('security', ['web']),
                               ('standard', ['web', 'bootui'])]:
            with self.subTest(feature=feature):
                original = self.packages['scafra']['features'][feature]
                self.packages['scafra']['features'][feature] = value
                self.assertTrue(any('feature/default' in e for e in self.errors()))
                self.packages['scafra']['features'][feature] = original
        self.packages['scafra']['features']['new-unreviewed-feature'] = []
        self.assertTrue(self.errors())

    def test_msrv_and_ci_drift(self):
        self.assertTrue(self.errors('0.0.0'))
        self.workspace['workspace']['package']['rust-version'] += '.1'
        self.assertTrue(any('MSRV' in e for e in self.errors()))

    def test_package_release_drift(self):
        self.workspace['workspace']['dependencies']['scafra-macros']['version'] += '-policy-drift'
        self.packages['scafra-core']['package']['version'] = '0.2.0'
        self.packages['scafra-web']['package']['rust-version'] = '1.89'
        self.packages['scafra']['dependencies']['scafra-core'] = {'version': '0.2'}
        errors = self.errors()
        self.assertTrue(any('scafra-macros' in e for e in errors))
        self.assertTrue(any('inherit workspace version' in e for e in errors))
        self.assertTrue(any('inherit workspace rust-version' in e for e in errors))
        self.assertTrue(any('scafra -> scafra-core' in e for e in errors))

    def test_missing_inherited_dependency(self):
        del self.workspace['workspace']['dependencies']['scafra-foundation']
        self.assertTrue(any('missing inherited workspace dependency' in e for e in self.errors()))

    def test_aliased_dependency_version_and_inheritance(self):
        dependency = self.workspace['workspace']['dependencies'].pop('scafra-foundation')
        dependency['package'] = 'scafra-foundation'
        self.workspace['workspace']['dependencies']['foundation-alias'] = dependency
        for manifest in self.packages.values():
            dependencies = manifest.get('dependencies', {})
            if 'scafra-foundation' in dependencies:
                dependencies['foundation-alias'] = dependencies.pop('scafra-foundation')
        self.assertEqual(self.errors(), [])
        dependency['version'] += '-policy-drift'
        self.assertTrue(any('scafra-foundation dependency requirement' in e for e in self.errors()))

    def test_canonical_package_identity(self):
        self.workspace['workspace']['dependencies']['scafra-foundation']['package'] = 'scafra-core'
        self.assertTrue(any('resolves to wrong package' in e for e in self.errors()))

    def test_three_component_msrv(self):
        self.contract['msrv'] += '.0'
        self.workspace['workspace']['package']['rust-version'] = self.contract['msrv']
        self.assertEqual(self.errors(self.contract['msrv']), [])
        self.assertTrue(self.errors(self.contract['msrv'] + '.0'))

    def test_target_build_dependency_inheritance(self):
        self.packages['scafra']['target'] = {'cfg(unix)': {'build-dependencies': {
            'renamed': {'package': 'scafra-core', 'version': '0.1.0'}}}}
        self.assertTrue(any('scafra -> scafra-core' in e for e in self.errors()))

    def test_malformed_contract_shapes(self):
        for key, value in [('msrv', 188), ('packages', ['scafra', 'scafra']),
                           ('features', {'default': 'web'}), ('verified_target', '')]:
            with self.subTest(key=key):
                contract = copy.deepcopy(self.contract)
                contract[key] = value
                with self.assertRaises(ValueError):
                    policy.check(contract, self.workspace, self.packages)

    @contextmanager
    def fixture(self):
        # Keep disposable test copies inside the checkout; never rewrite live manifests.
        with tempfile.TemporaryDirectory(prefix='.issue16-policy-test-', dir=policy.ROOT) as directory:
            root = Path(directory)
            files = ['Cargo.toml', 'docs/api-stability.md', 'scripts/check-api-policy.py']
            files.extend(member + '/Cargo.toml' for member in self.workspace['workspace']['members'])
            for name in files:
                (root / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(policy.ROOT / name, root / name)
            yield root

    def test_invalid_contract_cli_fails_cleanly(self):
        for payload in ['msrv = [', 'msrv = 188',
                        'msrv="1.88"\npackages=[]\nfeatures={default="web"}']:
            with self.subTest(payload=payload), self.fixture() as root:
                (root / 'docs/api-stability.md').write_text(
                    '<!-- compatibility-contract -->\n```toml\n' + payload + '\n```\n')
                result = subprocess.run([sys.executable, str(root / 'scripts/check-api-policy.py')],
                                        text=True, capture_output=True)
                self.assertEqual(result.returncode, 1)
                self.assertIn('API policy inputs invalid:', result.stderr)
                self.assertNotIn('Traceback', result.stderr)

    def test_contract_requires_a_closing_fence(self):
        with self.fixture() as root:
            document = root / 'docs/api-stability.md'
            document.write_text(document.read_text().split('\n```\n', 1)[0])
            with self.assertRaisesRegex(ValueError, 'must be closed'):
                policy.read_inputs(root)

    def test_workflow_stable_and_prerelease_version_selection(self):
        workflow = (policy.ROOT / '.github/workflows/publish.yml').read_text()
        step = workflow.split('      - name: Select release or snapshot version\n', 1)[1]
        shell = textwrap.dedent(step.split('        run: |\n', 1)[1].split('\n      - name:', 1)[0])
        for snapshot in [None, '0.1.0-snapshot.16', '1.0.0-rc.1']:
            with self.subTest(snapshot=snapshot), self.fixture() as root:
                current = self.workspace['workspace']['package']['version']
                result = subprocess.run(['bash', '-e', '-c', shell], cwd=root,
                    env={'PATH': os.defpath, 'GITHUB_EVENT_NAME': 'workflow_dispatch' if snapshot else 'push',
                         'SNAPSHOT_VERSION': snapshot or '', 'GITHUB_REF_NAME': 'v' + current,
                         'GITHUB_OUTPUT': str(root / 'github-output')}, text=True, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                contract, workspace, packages = policy.read_inputs(root)
                self.assertEqual(policy.check(contract, workspace, packages), [])
                self.assertEqual(workspace['workspace']['package']['version'], snapshot or current)
                self.assertEqual((root / 'github-output').read_text().strip(),
                                 'version=' + (snapshot or current))

    def test_valid_coordinated_release_versions(self):
        for version in ['0.2.0', '1.0.0', '1.2.3', '1.2.3-beta.2']:
            with self.subTest(version=version):
                workspace = copy.deepcopy(self.workspace)
                workspace['workspace']['package']['version'] = version
                for alias, dependency in workspace['workspace']['dependencies'].items():
                    name = dependency.get('package', alias) if isinstance(dependency, dict) else alias
                    if name in self.packages:
                        dependency['version'] = version
                self.assertEqual(policy.check(self.contract, workspace, self.packages), [])

    def test_public_api_changes_are_outside_checker_scope(self):
        # The guard deliberately reads no Rust source: an API deletion can stay green.
        with self.fixture() as root:
            source = root / 'crates/scafra-core/src/lib.rs'
            source.parent.mkdir(parents=True)
            source.write_text('// Entire public API removed.\n')
            self.assertEqual(policy.check(*policy.read_inputs(root)), [])

    def test_member_omission(self):
        del self.packages['scafra-cli']
        self.assertTrue(any('package set' in e for e in self.errors()))


if __name__ == '__main__':
    unittest.main()
