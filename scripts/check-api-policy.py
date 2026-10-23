#!/usr/bin/env python3
"""Check the reviewed API-policy TOML contract against Cargo (Python 3.11+).

This is a drift guard, not a SemVer checker. Policy changes need human review.
CI passes the actual consumer matrix toolchain to --rust.
"""
import argparse
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def read_inputs(root=ROOT):
    document = (root / 'docs/api-stability.md').read_text()
    marker = '<!-- compatibility-contract -->\n```toml\n'
    if document.count(marker) != 1:
        raise ValueError('policy must contain exactly one compatibility-contract block')
    contract = tomllib.loads(document.split(marker, 1)[1].split('\n```', 1)[0])
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())
    packages = {}
    for member in workspace['workspace']['members']:
        manifest = tomllib.loads((root / member / 'Cargo.toml').read_text())
        packages[manifest['package']['name']] = manifest
    return contract, workspace, packages


def check(contract, workspace, packages, rust=None):
    errors = []
    shared = workspace['workspace']['package']
    if contract['msrv'] != shared['rust-version']:
        errors.append('documented MSRV differs from workspace rust-version')
    if set(contract['packages']) != set(packages):
        errors.append('documented package set differs from workspace members')
    actual_features = packages['scafra']['features']
    expected_features = contract['features']
    if ({k: sorted(v) for k, v in actual_features.items()} !=
            {k: sorted(v) for k, v in expected_features.items()}):
        errors.append('facade feature/default/forwarding contract differs from Cargo.toml')
    for name, manifest in packages.items():
        for field in ('version', 'rust-version'):
            if manifest['package'].get(field) != {'workspace': True}:
                errors.append(f'{name} must inherit workspace {field}')
    for name, dependency in workspace['workspace']['dependencies'].items():
        if name not in packages:
            continue
        if not isinstance(dependency, dict) or dependency.get('version') != shared['version']:
            errors.append(f'{name} dependency requirement must match coordinated workspace version')
    # Check all normal/build/dev and target-specific first-party edges too:
    # an accidentally hard-coded local requirement must not bypass coordination.
    def dependencies(table):
        for key in ('dependencies', 'build-dependencies', 'dev-dependencies'):
            yield from table.get(key, {}).items()
        for target in table.get('target', {}).values():
            yield from dependencies(target)
    for name, manifest in packages.items():
        for alias, dependency in dependencies(manifest):
            package = dependency.get('package', alias) if isinstance(dependency, dict) else alias
            if package in packages and (
                    not isinstance(dependency, dict) or
                    dependency.get('workspace') is not True):
                errors.append(f'{name} -> {package} must use coordinated workspace dependency')
    if rust is not None and rust not in (contract['msrv'], contract['msrv'] + '.0', 'stable'):
        errors.append(f'consumer CI toolchain {rust} differs from documented MSRV {contract["msrv"]}')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rust', help='consumer CI matrix toolchain (MSRV or stable)')
    args = parser.parse_args()
    try:
        errors = check(*read_inputs(), rust=args.rust)
    except (KeyError, ValueError, OSError) as error:
        parser.exit(1, f'API policy inputs invalid: {error}\n')
    if errors:
        parser.exit(1, '\n'.join(errors) + '\nUpdate the policy and manifests together after compatibility review.\n')
    print('API policy matches Cargo features, defaults, MSRV and coordinated package versions.')


if __name__ == '__main__':
    main()
