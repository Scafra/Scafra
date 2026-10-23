"""Mutation tests: ensure real contract drift cannot silently pass the guard."""
import copy
import importlib.util
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
        return policy.check(self.contract, self.workspace, self.packages,
                            rust or self.contract['msrv'] + '.0')

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

    def test_member_omission(self):
        del self.packages['scafra-cli']
        self.assertTrue(any('package set' in e for e in self.errors()))


if __name__ == '__main__':
    unittest.main()
