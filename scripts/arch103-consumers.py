#!/usr/bin/env python3
"""Verify modular Scafra in isolated external Cargo projects (Python 3.11+).

Outputs must be a new directory outside the checkout. Each consumer resolves
independently; target artifacts may be shared without unifying Cargo features.
"""
import argparse
import base64
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
OPTIONAL = {"scafra-security", "scafra-actuator", "scafra-scheduler", "scafra-bootui", "jsonwebtoken", "reqwest"}
CASES = [
    ("minimal", "scafra", [], OPTIONAL | {"scafra-web", "axum"}),
    ("config", "scafra-config", [], OPTIONAL | {"scafra-web", "axum"}),
    ("core", "scafra-core", [], OPTIONAL | {"scafra-web", "axum"}),
    ("web", "scafra", ["web"], OPTIONAL),
    ("secure-web", "scafra", ["security"], OPTIONAL - {"scafra-security"}),
    ("inventory", "scafra", ["security"], OPTIONAL - {"scafra-security"}),
    ("jwt", "scafra", ["jwt"], {"reqwest", "scafra-actuator", "scafra-scheduler", "scafra-bootui"}),
    ("nested", "scafra", ["jwt"], {"reqwest", "scafra-actuator", "scafra-scheduler", "scafra-bootui"}),
    ("jwks", "scafra", ["jwks"], {"scafra-actuator", "scafra-scheduler", "scafra-bootui"}),
    ("actuator", "scafra", ["actuator"], OPTIONAL - {"scafra-actuator"}),
    ("scheduler", "scafra", ["scheduler"], OPTIONAL - {"scafra-scheduler"} | {"scafra-web", "axum"}),
    ("default", "scafra", None, OPTIONAL | {"scafra-web", "axum"}),
    ("standard", "scafra", ["standard"], {"scafra-bootui"}),
    ("bootui", "scafra", ["bootui"], OPTIONAL - {"scafra-bootui", "scafra-security"}),
    ("all", "scafra", ["standard", "bootui"], set()),
]

RESOURCE = '''use scafra::prelude::*;
pub struct Managed;
fn event(value: &str) { if let Ok(path) = std::env::var("ARCH103_EVENTS") { use std::io::Write; writeln!(std::fs::OpenOptions::new().append(true).create(true).open(path).unwrap(), "{value}").unwrap(); } }
impl AsyncResource for Managed {
 type ShutdownError = std::convert::Infallible;
 fn request_shutdown(&self, _: ShutdownReason) { event("cancel"); }
 async fn shutdown(&self, _: ShutdownReason, _: ShutdownPolicy) -> std::result::Result<(), Self::ShutdownError> { event("shutdown"); Ok(()) }
}
impl Drop for Managed { fn drop(&mut self) { event("drop"); } }
#[bean]
pub async fn managed() -> Managed { event("init"); Managed }
'''
PROPERTIES = '''use scafra::prelude::*;
#[derive(Default, serde::Serialize, serde::Deserialize, Config)]
#[config(prefix="mail")]
pub struct Mail { pub host: String }
#[service]
pub struct Service;
#[bean]
pub fn number() -> Number { Number(103) }
pub struct Number(pub u32);
'''
SCHEDULER = '''use std::sync::atomic::{AtomicUsize, Ordering};
static RUNS: AtomicUsize = AtomicUsize::new(0);
fn tick() { RUNS.fetch_add(1, Ordering::SeqCst); }
scafra::register_scheduled_task!("arch103", 5, tick);
#[tokio::main]
async fn main() {
 let mut app = scafra::Application::new(Default::default()); app.start().unwrap();
 let scheduler = scafra::Scheduler::try_start(&scafra::SchedulerConfig { enabled: true, ..Default::default() }).unwrap();
 tokio::time::timeout(std::time::Duration::from_secs(2), async { while RUNS.load(Ordering::SeqCst) == 0 { tokio::task::yield_now().await; } }).await.unwrap();
 scheduler.shutdown().await; let count = RUNS.load(Ordering::SeqCst);
 tokio::time::sleep(std::time::Duration::from_millis(20)).await; assert_eq!(RUNS.load(Ordering::SeqCst),count);
 app.shutdown().unwrap();
}
'''


# An actual supplied-router entrypoint, with the same macro-produced handler
# nested both before and after finalization. No URI-derived policies in fixture.
NESTED = r'''use scafra::prelude::*;
#[controller("/protected", roles("admin"))]
#[derive(Default)]
pub struct Protected;
#[routes]
impl Protected {
 #[get("/")] pub async fn root(&self) -> &'static str { "admin data" }
 #[get("/scoped")] #[scopes("reports.read")] pub async fn scoped(&self) -> &'static str { "scoped data" }
 #[get("/open")] #[public] pub async fn open(&self) -> &'static str { "public" }
}
#[controller("/implicit")]
#[derive(Default)]
pub struct Inherited;
#[routes]
impl Inherited {
 #[get("/public")] pub async fn public_name(&self) -> &'static str { "implicit" }
}
#[tokio::main]
async fn main() {
 scafra::run_with_finalized_router(|config| {
  let options = scafra::router_options(config);
  let routes = <Protected as scafra::web::ControllerRoutes>::route_metadata();
  let base = <Protected as scafra::web::ControllerRoutes>::register_routes(scafra::web::axum::Router::new());
  let finalized = scafra::web::finish_managed_router(base.clone(), routes, &options)?.nest("/prefinalized").into_router();
  let inherited = <Inherited as scafra::web::ControllerRoutes>::register_routes(scafra::web::axum::Router::new());
  let router = scafra::web::axum::Router::new().nest("/v2", base).merge(finalized).nest("/admin", inherited);
  let routes: Vec<_> = routes.iter().chain(<Inherited as scafra::web::ControllerRoutes>::route_metadata()).copied().collect();
  Ok(scafra::web::finish_managed_router(router, &routes, &options)?)
 }).await.unwrap();
}
'''


def runtime_nested(project, binary, env, jwt_enabled=None):
    if jwt_enabled is None:
        return {"shared_bearer": runtime_nested(project, binary, env, False),
                "jwt": runtime_nested(project, binary, env, True)}
    resources = project / "src/resources"
    resources.mkdir(exist_ok=True)
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    (resources / "application.yaml").write_text(
        f"server:\n  host: 127.0.0.1\n  port: {port}\nstartup:\n  banner: false\n"
        "security:\n  enabled: true\n  bearer_token: arch103-token\n  permit_all: [/implicit/public]\n"
        + ("  jwt:\n    enabled: true\n    secret: arch103-signing-secret\n" if jwt_enabled else ""))
    env = {k: v for k, v in env.items() if not k.startswith("SCAFRA_")}
    env.update(SCAFRA_CONFIG_DIR=str(resources), SCAFRA_DEV_RELOAD="false")
    checks = {}
    with (project / ("nested-jwt.log" if jwt_enabled else "nested-bearer.log")).open("w") as log:
        child = subprocess.Popen([str(binary)], cwd=project, env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 15
            while True:
                if child.poll() is not None:
                    raise AssertionError("nested consumer exited before readiness; see runtime.log")
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=.3):
                        break
                except OSError:
                    pass
                if time.monotonic() >= deadline:
                    raise AssertionError("nested consumer failed readiness; see runtime.log")
                time.sleep(.01)
            scenarios = [
                ("anonymous", "", None, 401),
                ("invalid", "", "wrong-token", 401),
                ("jwt-missing-role", "", jwt(), 403),
                ("jwt-admin", "", jwt(roles=["admin"]), 200),
                ("jwt-invalid-signature", "", jwt("wrong-secret", roles=["admin"]), 401),
                ("jwt-missing-scope", "/scoped", jwt(roles=["admin"]), 403),
                ("jwt-missing-role-with-scope", "/scoped", jwt(scope="reports.read"), 403),
                ("jwt-role-and-scope", "/scoped", jwt(roles=["admin"], scope="reports.read"), 200),
                ("public", "/open", None, 200),
            ]
            if not jwt_enabled:
                scenarios = [item for item in scenarios if not item[0].startswith("jwt-")]
                scenarios.insert(2, ("shared-bearer-missing-role", "", "arch103-token", 403))
            for prefix in ["/v2", "/prefinalized"]:
                for label, suffix, token, expected in scenarios:
                    actual, body = request(port, prefix + "/protected" + suffix, token)
                    assert actual == expected, f"{prefix} {label}: expected {expected}, got {actual}: {body!r}"
                    checks[prefix + ":" + label] = actual
            assert request(port, "/admin/implicit/public")[0] == 401, "nested inherited route matched stripped permit_all path"
            checks["original_uri_permit_all"] = 401
            child.send_signal(signal.SIGTERM)
            checks["sigterm_exit"] = child.wait(timeout=15)
            assert checks["sigterm_exit"] == 0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=5)
    return checks


# Linked module is deliberately outside src/main discovery: these adapters must
# pass through inventory registration in the actual generated main graph.
INVENTORY = r'''use scafra::prelude::*;
use scafra::web::{axum::{Router, routing::get}, ControllerRegistration, RouteMetadata};
fn register(router: Router) -> Router {
 router.route("/manual", get(|| async { "handwritten inventory" }))
}
const MANUAL_ROUTES: &[RouteMetadata] = &[RouteMetadata {
 controller: "ManualInventory", method: "GET", prefix: "/manual", path: "/"
}];
scafra::web::__private::inventory::submit! {
 ControllerRegistration { controller: "ManualInventory", register, routes: MANUAL_ROUTES }
}
#[controller("/linked", authenticated)]
#[derive(Default)]
pub struct Linked;
#[routes(default)]
impl Linked {
 #[get("/")] pub async fn root(&self) -> &'static str { "linked inventory" }
 #[get("/admin")] #[roles("admin")] pub async fn admin(&self) -> &'static str { "admin" }
 #[get("/open")] #[public] pub async fn open(&self) -> &'static str { "public" }
}
'''


def request(port, path, token=None):
    headers = {"Authorization": "Bearer " + token} if token else {}
    try:
        with urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}{path}", headers=headers), timeout=.3) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()


def jwt(secret="arch103-signing-secret", roles=(), scope=""):
    encode = lambda value: base64.urlsafe_b64encode(value).rstrip(b"=")
    header = encode(b'{"alg":"HS256","typ":"JWT"}')
    claims = encode(json.dumps({"sub": "test", "exp": int(time.time()) + 600, "roles": roles, "scope": scope}).encode())
    message = header + b"." + claims
    return (message + b"." + encode(hmac.new(secret.encode(), message, hashlib.sha256).digest())).decode()


def runtime(project, binary, name, env):
    if name == "nested":
        return runtime_nested(project, binary, env)
    if name in {"minimal", "default", "core", "config", "scheduler"}:
        subprocess.run([str(binary)], cwd=project, env=env, check=True, timeout=10)
        return {"exit_code": 0}
    secure = name in {"secure-web", "inventory", "jwt", "jwks", "standard", "all", "bootui"}
    has_jwt = name in {"jwt", "jwks"}
    actuator = name in {"actuator", "standard", "all"}
    configuration = "startup:\n  banner: false\n"
    bootui_port = None
    if name in {"all", "bootui"}:
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            bootui_port = reservation.getsockname()[1]
        configuration += f"bootui:\n  enabled: true\n  port: {bootui_port}\n"

    if secure:
        configuration += ("security:\n  enabled: true\n  jwt:\n    enabled: true\n    secret: arch103-signing-secret\n" if has_jwt else "security:\n  enabled: true\n  bearer_token: arch103-token\n")
    if name in {"standard", "all"}:
        configuration += "scheduler:\n  enabled: true\n"
    if actuator:
        configuration += "actuator:\n  endpoints: [health,live,ready]\n  security:\n    enabled: true\n    bearer_token: arch103-token\n"
    resources = project / "src/resources"
    resources.mkdir(exist_ok=True)
    config = resources / "application.yaml"
    events = project / "events.txt"
    base_env = {k: v for k, v in env.items() if not k.startswith("SCAFRA_")}
    base_env.update(SCAFRA_DEV_RELOAD="false", SCAFRA_CONFIG_DIR=str(resources), ARCH103_EVENTS=str(events))
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    config.write_text(f"server:\n  host: 127.0.0.1\n  port: {port}\n" + configuration)
    token = jwt() if has_jwt else "arch103-token" if secure else None
    with (project / "runtime.log").open("w") as log:
        child = subprocess.Popen([str(binary)], cwd=project, env=base_env, stdout=log, stderr=subprocess.STDOUT)
        try:
            start = time.monotonic()
            while True:
                if child.poll() is not None:
                    raise AssertionError(f"{name} exited early; see {project}/runtime.log")
                try:
                    status, body = request(port, "/probe", token)
                    if status == 200:
                        break
                except OSError:
                    pass
                if time.monotonic() - start > 15:
                    raise AssertionError(f"{name} did not become ready; see runtime.log")
                time.sleep(.01)
            assert json.loads(body) == {"ok": True}
            checks = {"http": 200, "json": True}
            if secure:
                assert request(port, "/probe")[0] == 401
                assert request(port, "/probe", "wrong-token")[0] == 401
                assert request(port, "/probe/admin", token)[0] == 403
                assert request(port, "/probe/open")[0] == 200
                checks.update(anonymous=401, invalid_token=401, missing_role=403, public=200)
            if name == "inventory":
                for path in ["/manual", "/linked"]:
                    assert request(port, path)[0] == 401, path
                    assert request(port, path, "wrong-token")[0] == 401, path
                    assert request(port, path, token)[0] == 200, path
                assert request(port, "/linked/admin", token)[0] == 403
                assert request(port, "/linked/open")[0] == 200
                checks["handwritten_inventory"] = {"anonymous": 401, "invalid": 401, "authenticated": 200}
                checks["macro_default_inventory"] = {"anonymous": 401, "authenticated": 200, "missing_role": 403, "public": 200}
            if has_jwt:
                assert request(port, "/probe", jwt("wrong-secret"))[0] == 401
                checks["invalid_signature"] = 401
            if actuator:
                # The operational token policy is independent of global security.
                for path in ["/health", "/live", "/ready", "/actuator/health/readiness"]:
                    assert request(port, path)[0] in {401, 404}
                    assert request(port, path, "wrong-operational-token")[0] in {401, 404}
                    assert request(port, path, "arch103-token")[0] == 200
                checks["actuator"] = True
                checks["actuator_invalid_token"] = "rejected"
            if bootui_port:
                dashboard_start = time.monotonic()
                while True:
                    try:
                        dashboard_status, snapshot = request(bootui_port, "/bootui/api/config")
                        if dashboard_status == 200:
                            break
                    except OSError:
                        pass
                    if time.monotonic() - dashboard_start > 10:
                        raise AssertionError("BootUI did not become ready")
                    time.sleep(.01)
                snapshot = json.loads(snapshot)
                assert set(snapshot) == {"server", "profile"}, snapshot
                checks["bootui_safe_snapshot"] = True
            child.send_signal(signal.SIGTERM)
            assert child.wait(timeout=15) == 0
            history = events.read_text().splitlines()
            assert history[0] == "init" and history[-1] == "drop", history
            assert history.count("shutdown") == 1 and history.index("cancel") < history.index("shutdown"), history
            if name in {"standard", "all"}:
                assert "tick" in history, history
            checks["resource_events"] = history
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()

    def failure(label, extra, expected_resource_cleanup=False):
        events.unlink(missing_ok=True)
        config.write_text(f"server:\n  port: {port}\n" + extra)
        with (project / (label + ".log")).open("w") as log:
            result = subprocess.run([str(binary)], cwd=project, env=base_env, stdout=log, stderr=subprocess.STDOUT, timeout=10)
        assert result.returncode != 0, label
        if expected_resource_cleanup:
            history = events.read_text().splitlines()
            assert history[0] == "init" and "shutdown" in history and history[-1] == "drop", history
        else:
            assert not events.exists(), f"{label}: resources initialized before config rejection"
        assert "secret-sentinel" not in (project / (label + ".log")).read_text()
        checks[label] = "rejected"

    failure("unknown-framework-key", "startup:\n  bannre: secret-sentinel\n")
    if not secure:
        failure("unavailable-security", "security:\n  enabled: true\n  bearer_token: secret-sentinel\n")
    elif not has_jwt and name == "secure-web":
        failure("unavailable-jwt", "security:\n  jwt:\n    enabled: true\n    secret: secret-sentinel\n")
        # Protected annotations with runtime-disabled security reject after graph init.
        failure("disabled-security", "", True)
    if name == "jwt":
        failure("unavailable-jwks", "security:\n  jwt:\n    enabled: true\n    jwk_set_uri: http://127.0.0.1:1/secret-sentinel\n")
    if actuator:
        failure("unknown-readiness", ("security:\n  enabled: true\n  bearer_token: arch103-token\n" if secure else "") + "actuator:\n  endpoints: [ready]\n  health:\n    checks: [unknown]\n", True)
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--proposed", action="store_true", help="compatibility flag; the contract is now implemented")
    parser.add_argument("--case", choices=[case[0] for case in CASES])
    parser.add_argument("--runtime", action="store_true")
    parser.add_argument("--timeout", type=int, default=240,
                        help="per Cargo command timeout in seconds")
    parser.add_argument("--target-dir", type=Path, help="reuse a Cargo target outside the checkout")
    args = parser.parse_args()
    out = args.output.resolve()
    if out == ROOT or ROOT in out.parents:
        parser.error("output must be outside checkout")
    out.mkdir(parents=True, exist_ok=False)
    results = []
    env = os.environ.copy()
    target = args.target_dir.resolve() if args.target_dir else out / "target"
    if target == ROOT or ROOT in target.parents:
        parser.error("target must be outside checkout")
    env["CARGO_TARGET_DIR"] = str(target)
    host = re.search(r"^host: (.*)$", subprocess.check_output(["rustc", "-Vv"], text=True), re.M).group(1)
    for name, dependency, features, forbidden in CASES:
        if args.case and args.case != name:
            continue
        project = out / name
        (project / "src").mkdir(parents=True)
        options = f'path = {json.dumps(str(ROOT / "crates" / dependency))}'
        if features is not None:
            options += ", default-features = false, features = " + json.dumps(features)
        manifest = (f'[package]\nname = "arch103-{name}"\nversion = "0.0.0"\nedition = "2021"\n'
                    f'[workspace]\n[dependencies]\n{dependency} = {{ {options} }}\n')
        web = name in {"web", "secure-web", "inventory", "jwt", "jwks", "actuator", "standard", "all", "bootui"}
        if dependency == "scafra-config":
            source = 'fn main() { let _ = scafra_config::ConfigLoader::new().env_prefix("ARCH103_UNUSED").load::<scafra_config::ScafraConfig>().unwrap(); }\n'
        elif dependency == "scafra-core":
            source = 'fn main() { let mut app = scafra_core::Application::new(Default::default()); app.start().unwrap(); app.shutdown().unwrap(); }\n'
        elif name == "scheduler":
            manifest += 'tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }\n'
            source = SCHEDULER
        elif name == "nested":
            manifest += 'tokio = { version = "1", features = ["rt-multi-thread", "macros"] }\n'
            source = NESTED
        elif web:
            manifest += 'serde_json = "1"\n'
            source = '#[scafra::main]\nasync fn main() {}\n'
            (project / "src/main/controllers").mkdir(parents=True)
            protected = ", authenticated" if name in {"secure-web", "inventory", "jwt", "jwks", "standard", "all", "bootui"} else ""
            (project / "src/main/controllers/probe.rs").write_text(
                f'use scafra::prelude::*;\n#[controller("/probe"{protected})]\npub struct Probe;\n'
                '#[routes]\nimpl Probe { #[get("/")] pub async fn get(&self) -> Json<serde_json::Value> { Json(serde_json::json!({"ok":true})) }\n'
                '#[get("/open")] #[public] pub async fn open(&self)-> &\'static str { "public" }\n'
                '#[get("/admin")] #[roles("admin")] pub async fn admin(&self)-> &\'static str { "admin" }\n}\n'
                if protected else
                'use scafra::prelude::*;\n#[controller("/probe")]\npub struct Probe;\n#[routes]\nimpl Probe { #[get("/")] pub async fn get(&self) -> Json<serde_json::Value> { Json(serde_json::json!({"ok":true})) } }\n')
            if name == "inventory":
                source = "mod linked;\n" + source
                (project / "src/linked.rs").write_text(INVENTORY)
            (project / "src/main/beans").mkdir()
            (project / "src/main/beans/resource.rs").write_text(RESOURCE)
            if name in {"standard", "all"}:
                (project / "src/main/jobs").mkdir()
                (project / "src/main/jobs/task.rs").write_text('pub fn tick() { use std::io::Write; if let Ok(path) = std::env::var("ARCH103_EVENTS") { writeln!(std::fs::OpenOptions::new().append(true).create(true).open(path).unwrap(), "tick").unwrap(); } }\nscafra::register_scheduled_task!("probe", 5, tick);\n')

        else:
            manifest += 'serde = { version = "1", features = ["derive"] }\n[build-dependencies]\nscafra-build = { path = ' + json.dumps(str(ROOT / "crates/scafra-build")) + ' }\n'
            (project / "build.rs").write_text('fn main() { scafra_build::discover_graph("src/main").unwrap(); }\n')
            (project / "src/main").mkdir()
            (project / "src/main/components.rs").write_text(PROPERTIES)
            source = ('include!(concat!(env!("OUT_DIR"), "/scafra_modules.rs"));\nfn main() { '
                      'let graph = __scafra_graph::compose().unwrap(); let _ = graph.into_resources(); '
                      'let mail = scafra::Properties::<components::Mail>::with_loader(scafra::ConfigLoader::new().env_prefix("ARCH103_UNUSED").override_value("mail.host", "example.test")).load().unwrap(); assert_eq!(mail.host,"example.test"); }\n')
        (project / "Cargo.toml").write_text(manifest)
        (project / "src/main.rs").write_text(source)
        (project / "Cargo.lock").write_bytes((ROOT / "Cargo.lock").read_bytes())
        entry = {"case": name, "runtime_requested": args.runtime}
        commands = [
            ("lock", ["cargo", "generate-lockfile", "--offline"]),
            ("tree", ["cargo", "tree", "--offline", "--locked", "--target", host, "-e", "normal", "--prefix", "none"]),
            ("build" if args.runtime else "check", ["cargo", "build" if args.runtime else "check", "--offline", "--locked"]),
        ]
        for label, command in commands:
            try:
                process = subprocess.run(command, cwd=project, env=env, text=True,
                                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                         timeout=args.timeout)
            except subprocess.TimeoutExpired as error:
                output = error.stdout or b""
                if isinstance(output, bytes):
                    output = output.decode(errors="replace")
                process = subprocess.CompletedProcess(command, -1,
                    output + f"\nCOMMAND TIMED OUT after {args.timeout} seconds\n")
            (project / (label + ".log")).write_text(process.stdout)
            entry[label + "_exit_code"] = process.returncode
            if label == "tree" and not process.returncode:
                packages = set(re.findall(r"^(\S+) v\S+", process.stdout, re.M))
                entry["forbidden_present"] = sorted(packages & forbidden)
                entry["normal_packages_excluding_root"] = len(set(re.findall(r"^(\S+) (v\S+)", process.stdout, re.M))) - 1
            if process.returncode:
                break
        if args.runtime and entry.get("build_exit_code") == 0 and not entry.get("forbidden_present"):
            try:
                entry["runtime"] = runtime(project, target / "debug" / f"arch103-{name}", name, env)
            except Exception as error:
                entry["runtime_error"] = str(error)
        results.append(entry)
        (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(json.dumps(entry), flush=True)
    if any(r.get("build_exit_code" if args.runtime else "check_exit_code") != 0 or r.get("forbidden_present") or "runtime_error" in r for r in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
