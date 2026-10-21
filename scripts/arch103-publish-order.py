#!/usr/bin/env python3
"""Validate packaging/publishing arrays against every internal normal/build edge.

Read-only: no crate is packaged, uploaded, or published by this command.
Uses Cargo's manifest interpretation, including optional and target dependencies.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="validate only (the only supported operation)")
    parser.parse_args()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT, text=True
    ))
    packages = {package["name"]: package for package in metadata["packages"]
                if package["id"] in metadata["workspace_members"] and package["publish"] != []}
    workflow = (ROOT / ".github/workflows/publish.yml").read_text()
    arrays = re.findall(r"packages=\(\s*(.*?)\s*\)", workflow, re.DOTALL)
    errors = []
    if len(arrays) != 2:
        errors.append(f"expected packaging and publishing arrays; found {len(arrays)}")
    orders = [array.split() for array in arrays]
    if len(orders) == 2 and orders[0] != orders[1]:
        errors.append("packaging and publishing arrays differ")
    for index, order in enumerate(orders, start=1):
        if len(order) != len(set(order)):
            errors.append(f"array {index} contains duplicate packages")
        if set(order) != set(packages):
            errors.append(f"array {index} package mismatch: missing={sorted(set(packages) - set(order))}, unknown={sorted(set(order) - set(packages))}")
        position = {name: offset for offset, name in enumerate(order)}
        for name, package in packages.items():
            for dependency in package["dependencies"]:
                dep_name = dependency["name"]
                if dependency["kind"] == "dev" or dep_name not in packages:
                    continue
                if name in position and dep_name in position and position[dep_name] >= position[name]:
                    errors.append(f"array {index}: {dep_name} must precede {name} ({dependency['kind'] or 'normal'} dependency)")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"PASS: both publish arrays order all {len(packages)} packages before their internal normal/build dependents")
    return 0


if __name__ == "__main__":
    sys.exit(main())
