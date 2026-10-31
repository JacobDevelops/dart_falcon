"""Small adversarial Cargo-metadata fixtures; no compiler or shared cache."""
from copy import deepcopy
import unittest

from check_architecture import violations


def package(name, dependencies=()):
    return {'id': name, 'name': name, 'manifest_path': f'/workspace/{name}/Cargo.toml',
            'dependencies': list(dependencies)}


def dependency(name, **overrides):
    return {'name': name, 'path': f'/workspace/{name}', 'kind': None, 'target': None, **overrides}


class ArchitectureTests(unittest.TestCase):
    def setUp(self):
        self.data = {'workspace_members': ['syntax', 'parser'], 'packages': [
            package('syntax'), package('parser', [dependency('syntax')])]}
        self.allowed = {'syntax': [], 'parser': ['syntax']}

    def test_valid_dependency_and_third_party(self):
        self.data['packages'][0]['dependencies'].append(dependency('serde', path=None))
        self.assertEqual(violations(self.data, self.allowed), [])

    def test_renamed_conditional_build_edge_cannot_bypass_policy(self):
        for kind in (None, 'build'):
            with self.subTest(kind=kind):
                data = deepcopy(self.data)
                data['packages'][0]['dependencies'] = [dependency(
                    'parser', rename='innocent_alias', kind=kind, target='cfg(windows)')]
                result = '\n'.join(violations(data, self.allowed))
                self.assertIn('syntax -> parser', result)
                self.assertIn('production cycle', result)

    def test_dev_edges_do_not_constrain_runtime_graph(self):
        self.data['packages'][0]['dependencies'] = [dependency('parser', kind='dev')]
        self.assertEqual(violations(self.data, self.allowed), [])

    def test_new_member_and_unclassified_path_fail_closed(self):
        self.data['packages'].append(package('extra'))
        self.data['workspace_members'].append('extra')
        self.data['packages'][0]['dependencies'] = [dependency('other')]
        result = '\n'.join(violations(self.data, self.allowed))
        self.assertIn('unclassified=', result)
        self.assertIn('unclassified local dependency', result)

    def test_unknown_policy_name_is_not_a_silent_exception(self):
        self.allowed['syntax'] = ['typo']
        self.assertIn('unknown policy dependency', '\n'.join(violations(self.data, self.allowed)))


if __name__ == '__main__':
    unittest.main()
