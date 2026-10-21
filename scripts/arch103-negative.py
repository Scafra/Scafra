#!/usr/bin/env python3
"""Compile/process regressions for ARCH-01 capability failures and feature unions.

Python 3.11+, Cargo and cached workspace dependencies are required. Example:
  python3 scripts/arch103-negative.py --output /var/tmp/arch103-negative-run

Each consumer is outside the workspace. Existing output directories are refused;
logs, sources, locks and results.json remain available for review. A shared target
cache does not unify Cargo features across independent consumer invocations.
"""
import argparse
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
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


def dependency(package, features, alias=None):
    fields = [f'path = {json.dumps(str(ROOT / "crates" / package))}',
              'default-features = false', 'features = ' + json.dumps(features)]
    if alias:
        fields.append('package = ' + json.dumps(package))
    return f'{alias or package} = {{ ' + ', '.join(fields) + ' }\n'


def project(out, name, features, protected=False, unified=False, entrypoint=True):
    path = out / name
    (path / 'src').mkdir(parents=True)
    manifest = (f'[package]\nname = "arch103-{name}"\nversion = "0.0.0"\nedition = "2021"\n'
                '[workspace]\n[dependencies]\n' + dependency('scafra', features))
    body = ''
    if unified:
        manifest += dependency('scafra-config', ['security', 'actuator', 'scheduler'])
        manifest += dependency('scafra-web', ['security', 'actuator'])
        # Require the unified subcrate fields at compile time. The facade's own
        # feature mask must still refuse to configure these capabilities.
        body = 'let _ = scafra_web::RouterOptions { security: Default::default(), actuator: Default::default(), request_timeout: None };'
    (path / 'Cargo.toml').write_text(manifest)
    (path / 'Cargo.lock').write_bytes((ROOT / 'Cargo.lock').read_bytes())
    (path / 'src/main.rs').write_text('#[scafra::main]\nasync fn main() { ' + body + ' }\n')
    if entrypoint:
        controllers = path / 'src/main/controllers'
        controllers.mkdir(parents=True)
        annotation = ', authenticated' if protected else ''
        (controllers / 'probe.rs').write_text(
            'use scafra::prelude::*;\n' + f'#[controller("/probe"{annotation})]\npub struct Probe;\n'
            '#[routes]\nimpl Probe { #[get("/")] pub async fn get(&self) -> &\'static str { "ok" } }\n')
        beans = path / 'src/main/beans'
        beans.mkdir()
        (beans / 'resource.rs').write_text(RESOURCE)
    return path


def command(path, label, argv, env, timeout):
    start = time.monotonic()
    try:
        result = subprocess.run(argv, cwd=path, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        output, code = result.stdout, result.returncode
    except subprocess.TimeoutExpired as error:
        output = error.stdout or b''
        if isinstance(output, bytes):
            output = output.decode(errors='replace')
        output += '\nCOMMAND TIMED OUT\n'
        code = -1
    (path / (label + '.log')).write_text(output)
    return {'command': argv, 'exit_code': code, 'seconds': time.monotonic() - start}, output


def build(path, env, timeout, check=False):
    lock, _ = command(path, 'lock', ['cargo', 'generate-lockfile', '--offline'], env, timeout)
    if lock['exit_code']:
        raise AssertionError('lockfile generation failed; see lock.log')
    return command(path, 'build', ['cargo', 'check' if check else 'build', '--offline', '--locked'], env, timeout)


def rejected_process(path, binary, configuration, label, expected, env, cleanup):
    resources = path / 'src/resources'
    resources.mkdir(exist_ok=True)
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    (resources / 'application.yaml').write_text(
        f'server:\n  host: 127.0.0.1\n  port: {port}\nstartup:\n  banner: false\n' + configuration)
    events = path / (label + '-events.txt')
    runtime_env = {key: value for key, value in env.items() if not key.startswith('SCAFRA_')}
    runtime_env.update(SCAFRA_DEV_RELOAD='false', SCAFRA_CONFIG_DIR=str(resources), ARCH103_EVENTS=str(events))
    listener_seen = False
    start = time.monotonic()
    with (path / (label + '.log')).open('w') as log:
        child = subprocess.Popen([str(binary)], cwd=path, env=runtime_env, stdout=log, stderr=subprocess.STDOUT)
        try:
            while child.poll() is None:
                try:
                    with socket.create_connection(('127.0.0.1', port), timeout=.02):
                        listener_seen = True
                except OSError:
                    pass
                if time.monotonic() - start > 15:
                    raise AssertionError('startup did not fail within 15 seconds')
                time.sleep(.001)
            code = child.wait(timeout=1)
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=5)
    output = (path / (label + '.log')).read_text()
    history = events.read_text().splitlines() if events.exists() else []
    assert code != 0, 'unsupported capability unexpectedly started successfully'
    assert not listener_seen, 'startup failure exposed a TCP listener'
    assert expected in output, f'missing diagnostic {expected!r}; see {label}.log'
    if cleanup:
        assert history == ['init', 'cancel', 'shutdown', 'drop'], history
    else:
        assert not history, f'providers ran before unsupported configuration was rejected: {history}'
    return {'exit_code': code, 'listener_observed': listener_seen, 'resource_events': history,
            'expected_diagnostic': expected, 'seconds': time.monotonic() - start}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, default=Path('/var/tmp/scafra-arch103-negative-target'))
    parser.add_argument('--timeout', type=int, default=240, help='per Cargo command timeout in seconds')
    args = parser.parse_args()
    out, target = args.output.resolve(), args.target_dir.resolve()
    if out == ROOT or ROOT in out.parents or target == ROOT or ROOT in target.parents:
        parser.error('output and target directories must be outside the checkout')
    if Path('/var/tmp') not in target.parents:
        parser.error('Cargo target directory must be under /var/tmp')
    out.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env['CARGO_TARGET_DIR'] = str(target)
    env.pop('CARGO_BUILD_TARGET', None)
    results = []
    (out / 'environment.json').write_text(json.dumps({
        'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
        'cargo': subprocess.check_output(['cargo', '-V'], text=True).strip(),
        'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'target_dir': str(target),
        'offline': True,
        'profile': 'dev',
        'listener_observation': 'loopback connect polling every millisecond until process exit',
    }, indent=2) + '\n')

    def record(name, run):
        entry = {'case': name}
        try:
            entry.update(run())
            entry['status'] = 'PASS'
        except Exception as error:
            entry.update(status='FAIL', error=str(error))
        results.append(entry)
        (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        print(json.dumps(entry), flush=True)

    def no_web():
        path = project(out, 'no-web-main', [], entrypoint=False)
        result, log = build(path, env, args.timeout, check=True)
        assert result['exit_code'] != 0, 'HTTP main compiled without web'
        assert 'requires the `web` Cargo feature' in log, 'missing feature diagnostic; see build.log'
        return {'compiler': result, 'expected_diagnostic': 'requires the `web` Cargo feature'}
    record('main-requires-web', no_web)

    def no_web_controller():
        path = project(out, 'no-web-controller', [], entrypoint=False)
        (path / 'src/main.rs').write_text('#[scafra::controller("/probe")]\npub struct Probe;\nfn main() {}\n')
        result, log = build(path, env, args.timeout, check=True)
        assert result['exit_code'] != 0, 'controller compiled without web'
        assert 'requires the `web` Cargo feature' in log, 'missing controller feature diagnostic; see build.log'
        return {'compiler': result, 'expected_diagnostic': 'requires the `web` Cargo feature'}
    record('controller-requires-web', no_web_controller)

    def protected():
        path = project(out, 'protected-no-security', ['web'], protected=True)
        result, _ = build(path, env, args.timeout)
        assert result['exit_code'] == 0, 'protected web-only application failed compilation; see build.log'
        runtime = rejected_process(path, target / 'debug/arch103-protected-no-security', '',
                                   'runtime', 'protected routes require the security Cargo feature', env, True)
        return {'compiler': result, 'runtime': runtime}
    record('protected-route-fails-closed', protected)

    def unified():
        path = project(out, 'unified-subcrates', ['web'], unified=True)
        result, _ = build(path, env, args.timeout)
        assert result['exit_code'] == 0, 'unified subcrate application failed compilation; see build.log'
        scenarios = {}
        for module, configuration in [('security', 'security:\n  enabled: true\n  bearer_token: test-token\n'),
                                      ('actuator', 'actuator:\n  endpoints: [health]\n'),
                                      ('scheduler', 'scheduler:\n  enabled: true\n')]:
            try:
                scenarios[module] = rejected_process(path, target / 'debug/arch103-unified-subcrates',
                                                     configuration, module, 'module is unavailable', env, False)
            except Exception as error:
                scenarios[module] = {'error': str(error)}
        (path / 'scenarios.json').write_text(json.dumps(scenarios, indent=2) + '\n')
        assert not any('error' in scenario for scenario in scenarios.values()), 'capability rejection failed; see scenarios.json and runtime logs'
        return {'compiler': result, 'scenarios': scenarios}
    record('unified-subcrate-capabilities-stay-rejected', unified)

    def additive():
        path = out / 'additive-union'
        (path / 'src').mkdir(parents=True)
        helper = path / 'helper'
        (helper / 'src').mkdir(parents=True)
        (helper / 'Cargo.toml').write_text('[package]\nname="arch103-union-helper"\nversion="0.0.0"\nedition="2021"\n[dependencies]\n' + dependency('scafra', ['jwks'], alias='framework'))
        (helper / 'src/lib.rs').write_text('pub fn enabled() { let _ = framework::JwtConfig::default(); }\n')
        (path / 'Cargo.toml').write_text('[package]\nname="arch103-additive-union"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[dependencies]\n' + dependency('scafra', ['web']) + 'arch103-union-helper = { path="helper" }\n')
        (path / 'src/main.rs').write_text('fn main() { let _ = scafra::JwtConfig::default(); arch103_union_helper::enabled(); }\n')
        (path / 'Cargo.lock').write_bytes((ROOT / 'Cargo.lock').read_bytes())
        result, _ = build(path, env, args.timeout)
        assert result['exit_code'] == 0, 'additive feature-union compilation failed; see build.log'
        tree, log = command(path, 'tree', ['cargo', 'tree', '--offline', '--locked', '-e', 'normal', '--prefix', 'none'], env, args.timeout)
        assert tree['exit_code'] == 0, 'cargo tree failed'
        packages = set(re.findall(r'^(\S+) v\S+', log, re.M))
        assert {'scafra-security', 'jsonwebtoken', 'reqwest'} <= packages, 'JWKS union dependencies absent'
        features, feature_log = command(path, 'features', ['cargo', 'tree', '--offline', '--locked', '-e', 'features'], env, args.timeout)
        assert features['exit_code'] == 0 and 'scafra feature "jwks"' in feature_log, 'JWKS feature union missing'
        return {'compiler': result, 'tree': tree, 'features': features,
                'verified_present': sorted({'scafra-security', 'jsonwebtoken', 'reqwest'})}
    record('features-are-additive-across-aliased-helper', additive)
    if any(entry['status'] != 'PASS' for entry in results):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
