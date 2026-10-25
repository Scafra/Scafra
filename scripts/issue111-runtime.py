#!/usr/bin/env python3
"""Verify resolved operational values in generated #[scafra::main] applications.

A new checkout-local ignored output directory is supported. No registry publish
or remote mutation. Evidence and consumer source remain in OUTPUT.
"""
import argparse
import base64
import http.client
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
FEATURES = ['web', 'security', 'jwt', 'actuator', 'standard', 'all']
CONTROLLER = '''use scafra::prelude::*;
#[controller("/resource", public)]
pub struct Resource;
#[routes]
impl Resource {
    #[post("/raw")]
    pub async fn raw(&self, request: scafra::web::axum::extract::Request) -> &'static str {
        scafra::debug!("cfg111_debug_marker");
        let _ = scafra::web::axum::body::to_bytes(request.into_body(), usize::MAX).await;
        "consumed"
    }
    #[get("/slow")]
    pub async fn slow(&self) -> &'static str {
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        "done"
    }
}
'''


def request(port, path, body=None, chunks=False, headers=None):
    conn = http.client.HTTPConnection('127.0.0.1', port, timeout=4)
    try:
        conn.request('POST' if body is not None else 'GET', path, body=body,
                     encode_chunked=chunks, headers=headers or {})
        response = conn.getresponse()
        return response.status, dict(response.getheaders()), response.read()
    finally:
        conn.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--target-dir', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    target = args.target_dir.resolve() if args.target_dir else out / 'target'
    results = {'rust': subprocess.check_output(['rustc', '-Vv'], text=True),
               'base': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
               'cases': []}
    for feature in FEATURES:
        project = out / feature
        controllers = project / 'src/main/controllers'
        controllers.mkdir(parents=True)
        (project / 'src/main.rs').write_text('#[scafra::main]\nasync fn main() {}\n')
        (controllers / 'resource.rs').write_text(CONTROLLER)
        selected = ['standard', 'bootui'] if feature == 'all' else [feature]
        (project / 'Cargo.toml').write_text('[package]\nname="cfg111-live"\nversion="0.0.0"\nedition="2021"\n'
            '[workspace]\n[dependencies]\nscafra={path=' + json.dumps(str(ROOT / 'crates/scafra')) +
            ',default-features=false,features=' + json.dumps(selected) + '}\n'
            'tokio={version="1",features=["time"]}\n')
        env = {k: v for k, v in os.environ.items() if not k.startswith('SCAFRA_')}
        env.update(CARGO_TARGET_DIR=str(target), SCAFRA_DEV_RELOAD='false',
                   SCAFRA_STARTUP__BANNER='false', RUST_LOG='info')
        for label, cmd in [('lock', ['cargo', 'generate-lockfile', '--offline']),
                           ('tree', ['cargo', 'tree', '-e', 'normal', '--locked', '--offline']),
                           ('build', ['cargo', 'build', '--locked', '--offline'])]:
            with (project / (label + '.log')).open('w') as log:
                subprocess.run(cmd, cwd=project, env=env, stdout=log, stderr=subprocess.STDOUT,
                               check=True, timeout=600)
        tree = (project / 'tree.log').read_text()
        if feature == 'web':
            for forbidden in ['scafra-security', 'scafra-actuator', 'scafra-scheduler', 'reqwest']:
                assert forbidden not in tree, forbidden
        config = project / 'config'
        config.mkdir()
        env['SCAFRA_CONFIG_DIR'] = str(config)
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0))
            port = reservation.getsockname()[1]
        env['SCAFRA_SERVER__PORT'] = str(port)
        ui_port = None
        if feature == 'all':
            with socket.socket() as reservation:
                reservation.bind(('127.0.0.1', 0))
                ui_port = reservation.getsockname()[1]
            env.update(SCAFRA_BOOTUI__ENABLED='true', SCAFRA_BOOTUI__PORT=str(ui_port),
                       SCAFRA_BOOTUI__PATH='/ops', SCAFRA_SECURITY__BASIC__USERNAME='ui-user',
                       SCAFRA_SECURITY__BASIC__PASSWORD='cfg111-ui-password')
        modes = ['default', 'yaml', 'yml', 'properties', 'profile', 'environment']
        if feature == 'web':
            modes += ['banner', 'compact', 'debug']
        for mode in modes:
            default_budget = mode in ['default', 'banner', 'compact', 'debug']
            for path in config.iterdir():
                path.unlink()
            run_env = env.copy()
            if mode in ['banner', 'compact']:
                run_env['SCAFRA_STARTUP__BANNER'] = 'true'
                run_env['SCAFRA_STARTUP__SHOW_CONFIG'] = 'true' if mode == 'banner' else 'false'
            if mode == 'debug':
                run_env.pop('RUST_LOG', None)
                run_env['SCAFRA_LOGGING__LEVEL'] = 'debug'
            if mode in ['yaml', 'yml']:
                (config / ('application.' + mode)).write_text('server:\n  max_request_body_bytes: 8\n  timeout: 1\n  header_read_timeout_seconds: 1\nshutdown:\n  grace_period_seconds: 2\n')
            if mode == 'properties':
                (config / 'application.properties').write_text('server.max_request_body_bytes=8\nserver.timeout=1\nserver.header_read_timeout_seconds=1\nshutdown.grace_period_seconds=2\n')
            if mode in ['profile', 'environment']:
                (config / 'application.yaml').write_text('server:\n  max_request_body_bytes: 16\n  timeout: 2\n')
                if mode == 'profile':
                    (config / 'application-production.yaml').write_text('server:\n  max_request_body_bytes: 8\n  timeout: 1\n  header_read_timeout_seconds: 1\n')
                    run_env['SCAFRA_PROFILE'] = 'production'
                else:
                    run_env.update(SCAFRA_SERVER__MAX_REQUEST_BODY_BYTES='8', SCAFRA_SERVER__TIMEOUT='1',
                                   SCAFRA_SERVER__HEADER_READ_TIMEOUT_SECONDS='1')
            if feature in ['security', 'jwt', 'standard', 'all']:
                run_env.update(SCAFRA_SECURITY__ENABLED='true', SCAFRA_SECURITY__BEARER_TOKEN='cfg111-test-token')
            if feature in ['jwt', 'standard', 'all']:
                run_env.update(SCAFRA_SECURITY__JWT__ENABLED='true', SCAFRA_SECURITY__JWT__SECRET='cfg111-test-signing-key')
            if feature in ['actuator', 'standard', 'all']:
                run_env['SCAFRA_ACTUATOR__ENDPOINTS'] = 'ready,health'
            if feature == 'all' and mode == 'environment':
                run_env['SCAFRA_BOOTUI__LOCAL_ONLY'] = 'false'
                run_env['SCAFRA_SECURITY__PERMIT_ALL'] = '[/ops/api/config,/api/config]'
            with (project / (mode + '.log')).open('w') as log:
                child = subprocess.Popen([str(target / 'debug/cfg111-live')], cwd=project, env=run_env,
                                         stdout=log, stderr=subprocess.STDOUT)
                try:
                    until = time.monotonic() + 10
                    while True:
                        assert child.poll() is None, project / (mode + '.log')
                        try:
                            if request(port, '/resource/raw', b'x')[0] == 200:
                                break
                        except OSError:
                            pass
                        assert time.monotonic() < until, 'startup deadline'
                        time.sleep(.01)
                    status, headers, body = request(port, '/resource/raw', [b'1234', b'5678', b'9'], True)
                    assert status == (200 if default_budget else 413), (feature, mode, status, body)
                    if not default_budget:
                        assert headers['content-type'] == 'application/json'
                        assert json.loads(body) == {'code': 'payload_too_large', 'message': 'request body too large'}
                        with socket.create_connection(('127.0.0.1', port), timeout=4) as slow:
                            slow.sendall(b'GET /resource/slow HTTP/1.1\r\nHost: incomplete')
                            data = slow.recv(1024)
                            assert not data or data.startswith(b'HTTP/1.1 408'), data
                    else:
                        assert request(port, '/resource/raw', b'x' * 1048577)[0] == 413
                    status, _, body = request(port, '/resource/slow')
                    assert status == (200 if default_budget else 504), (mode, status, body)
                    if ui_port is not None:
                        until = time.monotonic() + 5
                        while True:
                            try:
                                ui_status, _, ui_body = request(ui_port, '/ops/api/config')
                                break
                            except OSError:
                                assert time.monotonic() < until, 'BootUI startup deadline'
                                time.sleep(.01)
                        if mode == 'environment':
                            assert ui_status == 401, (ui_status, ui_body)
                            ui_status, _, ui_body = request(ui_port, '/ops/api/config', headers={'Authorization': 'Basic ' + base64.b64encode(b'ui-user:cfg111-ui-password').decode()})
                        assert ui_status == 200, (ui_status, ui_body)
                        assert json.loads(ui_body) == {'server': {'host': '127.0.0.1', 'port': port},
                                                       'profile': 'production' if mode == 'profile' else 'default'}
                        if not default_budget:
                            assert request(ui_port, '/ops/api/config', b'123456789')[0] == 413
                    child.send_signal(signal.SIGTERM)
                    assert child.wait(timeout=5) == 0
                    if ui_port is not None:
                        with socket.socket() as closed:
                            assert closed.connect_ex(('127.0.0.1', ui_port)) != 0
                    results['cases'].append({'feature': feature, 'source': mode, 'result': 'PASSED'})
                    (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
                finally:
                    if child.poll() is None:
                        child.kill()
                        child.wait(timeout=5)
            text = (project / (mode + '.log')).read_text()
            assert all(secret not in text for secret in ['cfg111-test-token', 'cfg111-test-signing-key', 'cfg111-ui-password', base64.b64encode(b'ui-user:cfg111-ui-password').decode()])
            if mode == 'banner':
                assert f'  server       http://127.0.0.1:{port}' in text
            elif mode == 'compact':
                assert '\n  Scafra 0.1.0\n' in text
                assert f'  server       http://127.0.0.1:{port}' not in text
            elif mode == 'debug':
                assert 'cfg111_debug_marker' in text
            elif mode == 'default':
                assert 'cfg111_debug_marker' not in text
                assert '\n  Scafra 0.1.0\n' not in text
        print(feature, 'PASSED', flush=True)
    worker = out / 'scheduler-only'
    (worker / 'src').mkdir(parents=True)
    (worker / 'Cargo.toml').write_text('[package]\nname="cfg111-worker"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[dependencies]\nscafra={path=' + json.dumps(str(ROOT / 'crates/scafra')) + ',default-features=false,features=["scheduler"]}\ntokio={version="1",features=["macros","rt-multi-thread","time"]}\n')
    (worker / 'application.properties').write_text('scheduler.enabled=true\nscheduler.tasks.worker.tick.interval_ms=2\nshutdown.grace_period_seconds=2\n')
    (worker / 'src/main.rs').write_text('''use std::sync::atomic::{AtomicUsize,Ordering};
static RUNS:AtomicUsize=AtomicUsize::new(0);
fn tick(){RUNS.fetch_add(1,Ordering::SeqCst);}
scafra::register_scheduled_task!("worker.tick",60000,tick);
#[tokio::main]
async fn main(){
 let config=scafra::ConfigLoader::new().load_validated::<scafra::ScafraConfig>().unwrap();
 assert_eq!(config.scheduler.tasks["worker.tick"].interval_ms,Some(2));
 let scheduler=scafra::Scheduler::try_start(&config.scheduler).unwrap();
 tokio::time::timeout(std::time::Duration::from_secs(2),async{while RUNS.load(Ordering::SeqCst)==0{tokio::task::yield_now().await;}}).await.unwrap();
 scheduler.shutdown().await;let count=RUNS.load(Ordering::SeqCst);
 tokio::time::sleep(std::time::Duration::from_millis(20)).await;assert_eq!(RUNS.load(Ordering::SeqCst),count);
}
''')
    env = {k:v for k,v in os.environ.items() if not k.startswith('SCAFRA_')}
    env['CARGO_TARGET_DIR'] = str(target)
    for label, cmd in [('lock',['cargo','generate-lockfile','--offline']),
                       ('tree',['cargo','tree','-e','normal','--locked','--offline']),
                       ('runtime',['cargo','run','--locked','--offline'])]:
        with (worker / (label + '.log')).open('w') as log:
            subprocess.run(cmd,cwd=worker,env=env,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=600)
    tree = (worker / 'tree.log').read_text()
    for forbidden in ['axum','scafra-web','scafra-security','reqwest','jsonwebtoken']:
        assert forbidden not in tree, forbidden
    results['cases'].append({'feature':'scheduler-only','source':'properties','result':'PASSED'})
    (out / 'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print('scheduler-only PASSED',flush=True)


if __name__ == '__main__':
    main()
