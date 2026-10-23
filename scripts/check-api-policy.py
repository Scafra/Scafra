#!/usr/bin/env python3
"""Check the reviewed API-policy TOML contract against Cargo (Python 3.11+).

This is a drift guard, not a SemVer checker. Policy changes need human review.
CI passes the actual consumer matrix toolchain to --rust.
"""
import argparse
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def validate_contract(contract):
    """Reject malformed facts with a predictable diagnostic before comparison."""
    for key in ('msrv', 'verified_target'):
        if not isinstance(contract.get(key), str) or not contract[key]:
            raise ValueError(f'policy {key} must be a nonempty string')
    parts = contract['msrv'].split('.')
    if len(parts) not in (2, 3) or not all(p.isascii() and p.isdigit() for p in parts):
        raise ValueError('policy msrv must be a numeric Rust version')
    names = contract.get('packages')
    if (not isinstance(names, list) or not names or
            not all(isinstance(n, str) and n for n in names) or
            len(set(names)) != len(names)):
        raise ValueError('policy packages must be distinct nonempty names')
    features = contract.get('features')
    if not isinstance(features, dict) or 'default' not in features:
        raise ValueError('policy features must be a table including default')
    for name, forwarding in features.items():
        if (not isinstance(forwarding, list) or
                not all(isinstance(item, str) for item in forwarding)):
            raise ValueError(f'policy feature {name} must be an array of strings')


def read_inputs(root=ROOT):
    document = (root / 'docs/api-stability.md').read_text()
    marker = '<!-- compatibility-contract -->\n```toml\n'
    if document.count(marker) != 1:
        raise ValueError('policy must contain exactly one compatibility-contract block')
    block, closing, _ = document.split(marker, 1)[1].partition('\n```')
    if not closing:
        raise ValueError('policy compatibility-contract block must be closed')
    contract = tomllib.loads(block)
    validate_contract(contract)
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())
    packages = {}
    for member in workspace['workspace']['members']:
        manifest = tomllib.loads((root / member / 'Cargo.toml').read_text())
        packages[manifest['package']['name']] = manifest
    return contract, workspace, packages


def check(contract, workspace, packages, rust=None):
    validate_contract(contract)
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
    workspace_dependencies = workspace['workspace']['dependencies']
    for alias, dependency in workspace_dependencies.items():
        name = dependency.get('package', alias) if isinstance(dependency, dict) else alias
        if alias in packages and name != alias:
            errors.append(f'canonical workspace alias {alias} resolves to wrong package {name}')
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
            inherited = isinstance(dependency, dict) and dependency.get('workspace') is True
            effective = dependency
            if inherited:
                if alias not in workspace_dependencies:
                    errors.append(f'{name} -> {alias}: missing inherited workspace dependency')
                    continue
                effective = workspace_dependencies[alias]
            package = effective.get('package', alias) if isinstance(effective, dict) else alias
            if package in packages and not inherited:
                errors.append(f'{name} -> {package} must use coordinated workspace dependency')
    supported_compilers = {contract['msrv'], 'stable'}
    if len(contract['msrv'].split('.')) == 2:
        supported_compilers.add(contract['msrv'] + '.0')
    if rust is not None and rust not in supported_compilers:
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
