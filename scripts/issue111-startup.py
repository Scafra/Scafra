#!/usr/bin/env python3
"""Prove programmatic configuration reaches generated application runtime owners.

Consumers, logs and machine-readable evidence remain under the supplied output
directory. Use a fresh ignored checkout-local output directory.
"""
import argparse
import http.client
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
MAIN = '''use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
fn configured_loader() -> scafra::ConfigLoader {
    assert!(INITIALIZED.load(Ordering::SeqCst), "loader preceded main body");
    assert_eq!(CALLS.fetch_add(1, Ordering::SeqCst), 0, "loader called twice");
    println!("cfg111_loader_called_once_after_main");
    let loader = scafra::ConfigLoader::new()
        .override_value("server.host", "127.0.0.1")
        .override_value("server.port", std::env::var("CFG111_PORT").unwrap())
        .override_value("server.max_request_body_bytes", "8")
        .override_value("server.timeout", "1")
        .override_value("logging.level", "debug")
        .override_value("shutdown.grace_period_seconds", "2");
    if let Ok(port) = std::env::var("CFG111_UI_PORT") {
        loader.override_value("bootui.enabled", "true")
            .override_value("bootui.port", port)
            .override_value("bootui.path", "/ops")
    } else { loader }
}
#[scafra::main(config_loader = crate::configured_loader)]
async fn main() {
    assert!(!INITIALIZED.swap(true, Ordering::SeqCst));
    println!("cfg111_main_body_completed");
}
'''
CONTROLLER = '''use scafra::prelude::*;
#[controller("/resource", public)]
pub struct Resource;
#[routes]
impl Resource {
    #[get("/ready")]
    pub async fn ready(&self) -> &'static str { "ready" }
    #[post("/raw")]
    pub async fn raw(&self, request: scafra::web::axum::extract::Request) -> &'static str {
        scafra::debug!("cfg111_programmatic_debug_marker");
        let _ = scafra::web::axum::body::to_bytes(request.into_body(), usize::MAX).await;
        "consumed"
    }
    #[get("/slow")]
    pub async fn slow(&self) -> &'static str {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        "done"
    }
}
'''


def port():
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        return reservation.getsockname()[1]


def request(listener, path, body=None, chunked=False):
    connection = http.client.HTTPConnection('127.0.0.1', listener, timeout=4)
    try:
        connection.request('POST' if body is not None else 'GET', path,
                           body=body, encode_chunked=chunked)
        response = connection.getresponse()
        return response.status, dict(response.getheaders()), response.read()
    finally:
        connection.close()


def wait_ready(child, listener, path):
    deadline = time.monotonic() + 10
    while True:
        assert child.poll() is None, 'application exited during startup; inspect runtime.log'
        try:
            response = request(listener, path)
            if response[0] == 200:
                return response
        except OSError:
            pass
        assert time.monotonic() < deadline, 'application startup deadline'
        time.sleep(.01)


def verify_reference(output, target, results):
    reference = ROOT / 'docs/configuration-reference.md'
    examples = [example for example in re.findall(r'```rust\n(.*?)```', reference.read_text(), re.S)
                if '#[scafra::main(config_loader = crate::configured_loader)]' in example]
    assert len(examples) == 1, 'expected exactly one generated loader example in the reference'
    project = output / 'reference'
    controllers = project / 'src/main/controllers'
    controllers.mkdir(parents=True)
    (project / 'src/main.rs').write_text(examples[0])
    (controllers / 'resource.rs').write_text(CONTROLLER)
    (project / 'Cargo.toml').write_text(
        '[package]\nname="cfg111-reference"\nversion="0.0.0"\nedition="2021"\n'
        '[workspace]\n[dependencies]\nscafra={path=' + json.dumps(str(ROOT / 'crates/scafra')) +
        ',default-features=false,features=["web"]}\n'
        'tokio={version="1",features=["time"]}\n')
    resources = project / 'src/resources'
    resources.mkdir()
    (resources / 'application.yaml').write_text('server:\n  max_request_body_bytes: 32\n')
    (resources / 'application-production.yaml').write_text('server:\n  max_request_body_bytes: 64\n')
    listener = port()
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('SCAFRA_', 'CFG111_')) and key != 'RUST_LOG'}
    env.update(CARGO_TARGET_DIR=str(target), TMPDIR=str(output / 'tmp'),
               SCAFRA_DEV_RELOAD='false', SCAFRA_STARTUP__BANNER='false',
               SCAFRA_SERVER__HOST='127.0.0.1', SCAFRA_SERVER__PORT=str(listener),
               SCAFRA_SERVER__MAX_REQUEST_BODY_BYTES='128')
    for phase, command in [('lock', ['cargo', 'generate-lockfile', '--offline']),
                           ('build', ['cargo', 'build', '--locked', '--offline'])]:
        with (project / (phase + '.log')).open('w') as log:
            subprocess.run(command, cwd=project, env=env, stdout=log,
                           stderr=subprocess.STDOUT, check=True, timeout=600)
    with (project / 'runtime.log').open('w') as log:
        child = subprocess.Popen([str(target / 'debug/cfg111-reference')], cwd=project,
                                 env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            wait_ready(child, listener, '/resource/ready')
            assert request(listener, '/resource/raw', b'x' * 800)[0] == 200
            status, headers, payload = request(listener, '/resource/raw', b'x' * 801)
            assert status == 413, (status, payload)
            assert headers['content-type'] == 'application/json'
            assert json.loads(payload) == {'code': 'payload_too_large', 'message': 'request body too large'}
            child.send_signal(signal.SIGTERM)
            assert child.wait(timeout=10) == 0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=5)
    case = {
        'feature': 'web', 'source': 'literal configuration-reference.md generated loader example',
        'result': 'PASSED', 'verified': ['compiled exact documented source',
                                       'body800 200', 'body801 413', 'SIGTERM shutdown']}
    (project / 'results.json').write_text(json.dumps(case, indent=2) + '\n')
    results['cases'].append(case)
    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    print('reference PASSED', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--target-dir', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    target = args.target_dir.resolve() if args.target_dir else output / 'target'
    tmp = output / 'tmp'
    tmp.mkdir()
    results = {'rust': subprocess.check_output(['rustc', '-Vv'], text=True),
               'base': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
               'cases': []}
    for label, features in [('web', ['web']), ('all', ['standard', 'bootui'])]:
        project = output / label
        controllers = project / 'src/main/controllers'
        controllers.mkdir(parents=True)
        (project / 'src/main.rs').write_text(MAIN)
        (controllers / 'resource.rs').write_text(CONTROLLER)
        (project / 'Cargo.toml').write_text(
            '[package]\nname="cfg111-startup"\nversion="0.0.0"\nedition="2021"\n'
            '[workspace]\n[dependencies]\nscafra={path=' + json.dumps(str(ROOT / 'crates/scafra')) +
            ',default-features=false,features=' + json.dumps(features) + '}\n'
            'tokio={version="1",features=["time"]}\n')
        config = project / 'config'
        config.mkdir()
        (config / 'application.yaml').write_text(
            'server:\n  host: 0.0.0.0\n  port: 18080\n  max_request_body_bytes: 32\n'
            '  timeout: 4\nlogging:\n  level: info\nshutdown:\n  grace_period_seconds: 8\n')
        (config / 'application-production.yaml').write_text(
            'server:\n  port: 18081\n  max_request_body_bytes: 24\n  timeout: 3\n'
            'logging:\n  level: warn\nshutdown:\n  grace_period_seconds: 6\n')
        listener, ui = port(), port() if label == 'all' else None
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(('SCAFRA_', 'CFG111_')) and key != 'RUST_LOG'}
        env.update(CARGO_TARGET_DIR=str(target), TMPDIR=str(tmp),
                   SCAFRA_DEV_RELOAD='false', SCAFRA_CONFIG_DIR=str(config),
                   SCAFRA_PROFILE='production', SCAFRA_STARTUP__BANNER='false',
                   SCAFRA_SERVER__HOST='0.0.0.0', SCAFRA_SERVER__PORT='18082',
                   SCAFRA_SERVER__MAX_REQUEST_BODY_BYTES='16', SCAFRA_SERVER__TIMEOUT='2',
                   SCAFRA_LOGGING__LEVEL='off', SCAFRA_SHUTDOWN__GRACE_PERIOD_SECONDS='4',
                   CFG111_PORT=str(listener))
        if ui is not None:
            env['CFG111_UI_PORT'] = str(ui)
        for phase, command in [('lock', ['cargo', 'generate-lockfile', '--offline']),
                               ('tree', ['cargo', 'tree', '-e', 'normal', '--locked', '--offline']),
                               ('build', ['cargo', 'build', '--locked', '--offline'])]:
            with (project / (phase + '.log')).open('w') as log:
                subprocess.run(command, cwd=project, env=env, stdout=log,
                               stderr=subprocess.STDOUT, check=True, timeout=600)
        if label == 'web':
            tree = (project / 'tree.log').read_text()
            for forbidden in ['scafra-security', 'scafra-actuator', 'scafra-scheduler', 'reqwest']:
                assert forbidden not in tree, forbidden
        log_path = project / 'runtime.log'
        with log_path.open('w') as log:
            child = subprocess.Popen([str(target / 'debug/cfg111-startup')], cwd=project,
                                     env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                wait_ready(child, listener, '/resource/ready')
                assert request(listener, '/resource/raw', b'1234567')[0] == 200
                assert request(listener, '/resource/raw', b'12345678')[0] == 200
                for body, chunked in [(b'123456789', False), ([b'1234', b'5678', b'9'], True)]:
                    status, headers, payload = request(listener, '/resource/raw', body, chunked)
                    assert status == 413, (status, payload)
                    assert headers['content-type'] == 'application/json'
                    assert json.loads(payload) == {'code': 'payload_too_large', 'message': 'request body too large'}
                before = time.monotonic()
                status, headers, payload = request(listener, '/resource/slow')
                elapsed = time.monotonic() - before
                assert status == 504, (status, payload)
                assert .8 <= elapsed < 4, elapsed
                assert headers['content-type'] == 'application/json'
                assert json.loads(payload)['code'] == 'request_timeout'
                if ui is not None:
                    _, _, payload = wait_ready(child, ui, '/ops/api/config')
                    assert json.loads(payload) == {
                        'server': {'host': '127.0.0.1', 'port': listener}, 'profile': 'production'}
                before = time.monotonic()
                child.send_signal(signal.SIGTERM)
                assert child.wait(timeout=5) == 0
                shutdown_elapsed = time.monotonic() - before
                for bound in [listener] + ([ui] if ui is not None else []):
                    with socket.socket() as closed:
                        assert closed.connect_ex(('127.0.0.1', bound)) != 0
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait(timeout=5)
        text = log_path.read_text()
        assert text.count('cfg111_loader_called_once_after_main') == 1
        assert text.count('cfg111_main_body_completed') == 1
        assert text.index('cfg111_main_body_completed') < text.index('cfg111_loader_called_once_after_main')
        assert 'cfg111_programmatic_debug_marker' in text
        results['cases'].append({
            'feature': label, 'result': 'PASSED', 'profile': 'production',
            'sources': ['defaults', 'base YAML', 'profile YAML', 'environment', 'programmatic'],
            'verified': ['loader order and single invocation', 'listener address', 'body boundaries',
                         'chunked raw-body 413', 'processing 504', 'debug logging over environment off',
                         'SIGTERM shutdown'] + (['BootUI effective address and profile'] if ui else []),
            'timeout_observed_seconds': elapsed, 'shutdown_observed_seconds': shutdown_elapsed})
        (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        print(label, 'PASSED', flush=True)
    verify_reference(output, target, results)


if __name__ == '__main__':
    main()
