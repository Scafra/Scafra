#!/usr/bin/env python3
"""ARCH-01 Phase 0: inspect dependencies and measure the current generated API.

Python 3.11+, Cargo, and a populated Cargo cache are required. All output goes
to a NEW directory outside the checkout. No cargo clean or source edits.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import socket
import subprocess
import time
import tomllib
import urllib.request


ROOT = Path(__file__).resolve().parents[1]


def run(args, cwd=ROOT, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_hash():
    digest = hashlib.sha256()
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    paths += sorted((ROOT / "crates").glob("*/Cargo.toml"))
    paths += sorted((ROOT / "crates").glob("*/src/**/*.rs"))
    for path in paths:
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(path.read_bytes())
    return digest.hexdigest()


def packages(tree):
    # --prefix none prints package/version followed by optional path and (*).
    return sorted(set(re.findall(r"^([^\s]+) (v[^\s]+)", tree, re.M)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new output directory outside checkout")
    parser.add_argument("--features", default="web", help="CLI capability selection, e.g. web or standard")
    parser.add_argument("--measure", action="store_true", help="also build/run generated API")
    parser.add_argument("--consumer-lock", type=Path,
                        help="reuse an archived generated-api lock exactly")
    parser.add_argument("--refresh-consumer-lock", action="store_true",
                        help="allow Cargo to update internal package edges in a seed lock")
    args = parser.parse_args()
    out = args.output.resolve()
    if out == ROOT or ROOT in out.parents:
        parser.error("output must be outside checkout")
    out.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(out / "tool-target")
    # Do not print inherited environment: it may contain secrets.
    report = {
        "commit": run(["git", "rev-parse", "HEAD"]).strip(),
        "branch": run(["git", "branch", "--show-current"]).strip(),
        "working_tree": run(["git", "status", "--short"]),
        "rustc": run(["rustc", "-Vv"]),
        "cargo": run(["cargo", "-V"]).strip(),
        "platform": platform.platform(),
        "cpu_count": os.cpu_count(),
        "workspace_lock_sha256": sha(ROOT / "Cargo.lock"),
        "source_sha256": source_hash(),
        "profiles": "dev defaults; release thin LTO, codegen-units=1 (CLI template)",
        "measurements": [],
    }
    for name in ["cpuinfo", "meminfo"]:
        path = Path("/proc") / name
        if path.exists():
            (out / name).write_text(path.read_text())

    def save():
        (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    def timed(label, command, cwd=ROOT, command_env=env):
        start = time.monotonic()
        with (out / (label + ".log")).open("w") as log:
            result = subprocess.run(command, cwd=cwd, env=command_env,
                                    stdout=log, stderr=subprocess.STDOUT)
        record = {"label": label, "command": command, "exit_code": result.returncode,
                  "seconds": round(time.monotonic() - start, 6)}
        report["measurements"].append(record)
        save()
        if result.returncode:
            raise RuntimeError(f"{label} failed; see {out / (label + '.log')}")

    save()
    metadata = json.loads(run(["cargo", "metadata", "--locked", "--offline",
                               "--format-version", "1"]))
    (out / "metadata.json").write_text(json.dumps(metadata))
    members = set(metadata["workspace_members"])
    audit = []
    host = re.search(r"^host: (.*)$", report["rustc"], re.M).group(1)
    for package in sorted(metadata["packages"], key=lambda p: p["name"]):
        if package["id"] not in members:
            continue
        name = package["name"]
        trees = {}
        for edge in ["normal", "features"]:
            trees[edge] = run(["cargo", "tree", "--locked", "--offline", "-p", name,
                               "--target", host, "-e", edge, "--prefix", "none"])
            (out / f"{name}-{edge}.txt").write_text(trees[edge])
        audit.append({"name": name, "dependencies": package["dependencies"],
                      "features": package["features"],
                      "normal_closure_excluding_root": len(packages(trees["normal"])) - 1})
        timed("check-" + name, ["cargo", "check", "--locked", "--offline", "-p", name,
                                "--no-default-features"])
    (out / "audit.json").write_text(json.dumps(audit, indent=2) + "\n")
    if not args.measure:
        return
    timed("cli-build", ["cargo", "build", "--locked", "--offline", "-p", "scafra-cli"])
    project = out / "generated-api"
    timed("generate-api", [str(out / "tool-target/debug/scafra"), "new", str(project),
                            "--kind", "api", "--features", args.features])
    consumer_env = env.copy()
    consumer_env["CARGO_TARGET_DIR"] = str(out / "consumer-target")
    # The template uses the checked-out local facade. Preserve exact versions
    # from the workspace lock; Cargo normalizes the root package entry offline.
    seed_lock = args.consumer_lock.resolve() if args.consumer_lock else ROOT / "Cargo.lock"
    (project / "Cargo.lock").write_bytes(seed_lock.read_bytes())
    lock_command = (["cargo", "metadata", "--format-version", "1", "--offline"] + ([] if args.refresh_consumer_lock else ["--locked"])
                    if args.consumer_lock else ["cargo", "generate-lockfile", "--offline"])
    timed("consumer-lock", lock_command, project, consumer_env)
    report["consumer_lock_sha256"] = sha(project / "Cargo.lock")
    report["consumer_manifest"] = (project / "Cargo.toml").read_text()
    for edge in ["normal", "features"]:
        tree = run(["cargo", "tree", "--locked", "--offline", "--target", host,
                    "-e", edge, "--prefix", "none"], project, consumer_env)
        (out / f"consumer-{edge}.txt").write_text(tree)
        if edge == "normal":
            report["consumer_normal_closure_excluding_root"] = len(packages(tree)) - 1
    timed("consumer-clean-dev", ["cargo", "build", "--locked", "--offline"], project, consumer_env)
    timed("consumer-warm-noop", ["cargo", "build", "--locked", "--offline"], project, consumer_env)
    source = project / "src/main.rs"
    source.write_text(source.read_text() + "\n// ARCH-01 incremental probe\n")
    timed("consumer-incremental-edit", ["cargo", "build", "--locked", "--offline"], project, consumer_env)
    timed("consumer-release", ["cargo", "build", "--release", "--locked", "--offline"], project, consumer_env)
    package_name = tomllib.loads((project / "Cargo.toml").read_text())["package"]["name"]
    binary = out / "consumer-target/release" / package_name
    report["release_binary_bytes_unstripped"] = binary.stat().st_size
    # Use only controlled configuration; don't inherit arbitrary SCAFRA_ values.
    runtime_env = {k: v for k, v in consumer_env.items() if not k.startswith("SCAFRA_")}
    runtime_env["SCAFRA_DEV_RELOAD"] = "false"
    runtime_env["SCAFRA_CONFIG_DIR"] = str(project / "src/resources")
    runtime_results = []
    scenarios = [
        ("default", "", {}),
        ("secure", "security:\n  enabled: true\n  bearer_token: arch103-test-token\n",
         {"Authorization": "Bearer arch103-test-token"}),
        ("secure-actuator", "security:\n  enabled: true\n  bearer_token: arch103-test-token\nactuator:\n  endpoints: [health]\n",
         {"Authorization": "Bearer arch103-test-token"}),
    ]
    if "standard" not in args.features:
        scenarios = scenarios[:1]
    report["selected_capabilities"] = args.features
    for scenario, extra, headers in scenarios:
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        # Port reservation is released before the child binds; a conflict fails
        # visibly through exit status/deadline rather than being a timing sample.
        (project / "src/resources/application.yaml").write_text(
            f"server:\n  host: 127.0.0.1\n  port: {port}\nstartup:\n  banner: false\n" + extra)
        start = time.monotonic()
        with (out / f"runtime-{scenario}.log").open("w") as log:
            child = subprocess.Popen([str(binary)], cwd=project, env=runtime_env,
                                     stdout=log, stderr=subprocess.STDOUT)
            try:
                url = f"http://127.0.0.1:{port}/api/greetings/ARCH103"
                while True:
                    if child.poll() is not None:
                        raise RuntimeError(f"runtime {scenario} exited: see log")
                    try:
                        with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=.2) as response:
                            assert response.status == 200
                            assert json.load(response) == {"message": "Hello, ARCH103!"}
                        break
                    except (OSError, urllib.error.URLError):
                        if time.monotonic() - start > 15:
                            raise RuntimeError(f"runtime {scenario} readiness deadline exceeded")
                        time.sleep(.005)
                sample = {"scenario": scenario, "spawn_to_first_http_seconds": time.monotonic() - start}
                status = Path(f"/proc/{child.pid}/status")
                if status.exists():
                    sample["memory"] = [line for line in status.read_text().splitlines()
                                        if line.startswith(("VmRSS:", "VmHWM:"))]
                child.send_signal(signal.SIGTERM)
                sample["shutdown_exit_code"] = child.wait(timeout=15)
                runtime_results.append(sample)
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
        report["runtime"] = runtime_results
        save()
    report["workspace_lock_unchanged"] = sha(ROOT / "Cargo.lock") == report["workspace_lock_sha256"]
    report["source_unchanged_during_measurement"] = source_hash() == report["source_sha256"]
    save()
    if not report["source_unchanged_during_measurement"]:
        raise RuntimeError("framework source changed during measurement; rerun before comparing")


if __name__ == "__main__":
    main()
