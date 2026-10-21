#!/usr/bin/env python3
"""Verify packaged crates using a local directory registry source, without publishing.

Python 3.11+ and Cargo are required. OUTPUT must be a new directory outside the
checkout. Cargo vendors locked registry dependencies, then packages workspace
crates in publish order and stages their normalized archives into that source.
This tests extracted packages and registry-style dependency resolution, but does
not prove crates.io ownership, index propagation, or upload acceptance.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--offline", action="store_true", help="require cached external registry dependencies")
    parser.add_argument("--reuse", action="store_true", help="reuse a previous output created by this script")
    args = parser.parse_args()
    output = args.output.resolve()
    if output == ROOT or ROOT in output.parents:
        parser.error("output must be outside the checkout")
    marker = output / ".arch103-package-check"
    if output.exists():
        if not args.reuse or not marker.is_file():
            parser.error("output already exists; --reuse requires this script's marker")
    else:
        output.mkdir(parents=True)
        marker.touch()
    vendor = output / "vendor"
    target = output / "target"
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    results = {
        "method": "local directory registry-source staging; no publication",
        "rustc": subprocess.check_output(["rustc", "-vV"], text=True).strip(),
        "cargo": subprocess.check_output(["cargo", "--version"], text=True).strip(),
        "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "packages": {}, "consumers": {},
    }
    (output / "results.json").write_text(json.dumps(results, indent=2))

    def run(command, cwd=ROOT, label="cargo"):
        log = output / (label + ".log")
        with log.open("w") as stream:
            result = subprocess.run(command, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT)
        if result.returncode:
            raise RuntimeError(f"{' '.join(command)} failed ({result.returncode}); see {log}")
        print(f"PASS {label}", flush=True)

    offline = ["--offline"] if args.offline else []
    run(["python3", str(ROOT / "scripts/arch103-publish-order.py"), "--check"], label="publish-order")
    if not (output / ".vendor-complete").exists():
        run(["cargo", "vendor", "--locked", "--versioned-dirs", *offline, str(vendor)], label="vendor")
        (output / ".vendor-complete").touch()
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, env=env, text=True))
    packages = {package["name"]: package for package in metadata["packages"]}
    order = re.findall(r"packages=\(\s*(.*?)\s*\)", (ROOT / ".github/workflows/publish.yml").read_text(), re.DOTALL)[0].split()
    config = ["--config", 'source.crates-io.replace-with="arch103-staged"', "--config", f'source.arch103-staged.directory={json.dumps(str(vendor))}']
    for name in order:
        package = packages[name]
        # Build and verify the extracted archive, using only the staged registry
        # source for all dependencies. No workspace path patches are supplied.
        run(["cargo", "package", "-p", name, "--locked", "--allow-dirty", "--offline", *config], label=name)
        archive = target / "package" / f"{name}-{package['version']}.crate"
        with tarfile.open(archive) as source:
            for member in source.getmembers():
                destination = vendor / member.name
                if vendor not in destination.resolve().parents:
                    raise RuntimeError(f"unsafe archive path: {member.name}")
                if member.isdir():
                    destination.mkdir(parents=True, exist_ok=True)
                elif member.isfile():
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(source.extractfile(member).read())
                    destination.chmod(member.mode & 0o777)
                else:
                    raise RuntimeError(f"unsupported archive member: {member.name}")
        staged = vendor / f"{name}-{package['version']}"
        files = {str(path.relative_to(staged)): hashlib.sha256(path.read_bytes()).hexdigest()
                 for path in staged.rglob("*") if path.is_file() and path.name != ".cargo-checksum.json"}
        (staged / ".cargo-checksum.json").write_text(json.dumps({"files": files, "package": None}))
        results["packages"][name] = {"version": package["version"], "verification": "PASS extracted package"}
        (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    for feature in ["web", "standard"]:
        consumer = output / ("consumer-" + feature)
        (consumer / "src").mkdir(parents=True, exist_ok=True)
        version = packages["scafra"]["version"]
        (consumer / "Cargo.toml").write_text(f'[package]\nname="packaged-{feature}"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[dependencies]\nscafra={{version="={version}",default-features=false,features=["{feature}"]}}\n')
        (consumer / "src/main.rs").write_text("#[scafra::main]\nasync fn main() {}\n")
        run(["cargo", "check", "--offline", *config], cwd=consumer, label="consumer-" + feature)
        results["consumers"][feature] = "PASS registry-source macro consumer"
        (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(f"Verified all packaged crates and registry-source consumers; evidence: {output / 'results.json'}")


if __name__ == "__main__":
    main()
