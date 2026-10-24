#!/usr/bin/env python3
"""Build an isolated generated Scafra app and verify errors over loopback HTTP/1.

The output must be new. An ignored checkout-local target directory is allowed;
external output directories need the caller's authorization. Sources, lockfile,
Cargo trees, process logs and results remain available for inspection.
"""
import argparse
import http.client
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
SENTINEL = 'issue12-private-database://password/path'


def request(port, path, method='GET', body=None, headers=None):
    connection = http.client.HTTPConnection('127.0.0.1', port, timeout=4)
    try:
        connection.request(method, path, body=body, headers=headers or {})
        response = connection.getresponse()
        return response.status, response.getheaders(), response.read()
    finally:
        connection.close()


def contract(response, status, code, message):
    actual, headers, body = response
    assert actual == status, response
    assert dict(headers).get('content-type') == 'application/json', response
    assert json.loads(body) == {'code': code, 'message': message}, response
    assert SENTINEL.encode() not in body, response


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--target-dir', type=Path, help='reuse build artifacts without unifying features')
    parser.add_argument('--features', default='standard', choices=['web', 'security', 'jwt', 'actuator', 'standard', 'all'])
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    controllers = out / 'src/main/controllers'
    controllers.mkdir(parents=True)
    secure = args.features in {'security', 'jwt', 'standard', 'all'}
    jwt = args.features in {'jwt', 'standard', 'all'}
    actuator = args.features in {'actuator', 'standard', 'all'}
    features = ['standard', 'bootui'] if args.features == 'all' else [args.features]
    (out / 'Cargo.toml').write_text('[package]\nname = "issue12-live"\nversion = "0.0.0"\nedition = "2021"\n'
        '[workspace]\n[dependencies]\nscafra = { path = ' + json.dumps(str(ROOT / 'crates/scafra')) +
        ', default-features = false, features = ' + json.dumps(features) + ' }\nserde = { version = "1", features = ["derive"] }\n')
    (out / 'src/main.rs').write_text('#[scafra::main]\nasync fn main() {}\n')
    shutil.copyfile(ROOT / 'scripts/fixtures/http-errors/errors.rs', controllers / 'errors.rs')
    if secure:
        shutil.copyfile(ROOT / 'scripts/fixtures/http-errors/protected.rs', controllers / 'protected.rs')
    shutil.copyfile(ROOT / 'Cargo.lock', out / 'Cargo.lock')
    env = {k: v for k, v in os.environ.items() if not k.startswith('SCAFRA_')}
    target = args.target_dir.resolve() if args.target_dir else out / 'target'
    env.update(CARGO_TARGET_DIR=str(target), RUST_LOG='trace', SCAFRA_DEV_RELOAD='false')
    results = {'features': features, 'compiler': subprocess.check_output(['rustc', '-V'], text=True).strip(), 'checks': []}
    for label, command in [
        ('lock', ['cargo', 'generate-lockfile', '--offline']),
        ('tree', ['cargo', 'tree', '-e', 'normal', '--offline', '--locked']),
        ('build', ['cargo', 'build', '--offline', '--locked']),
    ]:
        with (out / (label + '.log')).open('w') as log:
            subprocess.run(command, cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=600)
    tree = (out / 'tree.log').read_text()
    if args.features == 'web':
        for package in ['scafra-security', 'scafra-actuator', 'scafra-scheduler', 'jsonwebtoken', 'reqwest']:
            assert package not in tree, package
    resources = out / 'src/resources'
    resources.mkdir()
    env['SCAFRA_CONFIG_DIR'] = str(resources)
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    base = f'server:\n  host: 127.0.0.1\n  port: {port}\n  timeout: 1\nstartup:\n  banner: false\n'
    modes = ['normal', 'hidden'] if secure else ['normal']
    if jwt:
        modes.append('jwt')
    for mode in modes:
        config = base
        if secure:
            config += 'security:\n  enabled: true\n  bearer_token: issue12-token\n'
            if actuator:
                config += '  permit_all: [/ready,/health,/live]\n'
            config += '  basic:\n    username: user\n    password: issue12-basic-password\n'
            if mode == 'hidden':
                config += '  hide_unauthorized: true\n'
            if mode == 'jwt':
                config += '  jwt:\n    enabled: true\n    secret: issue12-signing-secret\n'
        if actuator:
            config += 'actuator:\n  endpoints: [health,ready,live]\n  security:\n    enabled: true\n    bearer_token: issue12-actuator-token\n'
        (resources / 'application.yaml').write_text(config)
        logpath = out / f'{mode}.log'
        with logpath.open('w') as log:
            child = subprocess.Popen([str(target / 'debug/issue12-live')], cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                start = time.monotonic()
                while True:
                    assert child.poll() is None, f'early exit: {logpath}'
                    try:
                        if request(port, '/errors/typed')[0] == 400:
                            break
                    except OSError:
                        pass
                    assert time.monotonic() - start < 15, f'startup deadline: {logpath}'
                    time.sleep(.01)
                if mode == 'normal':
                    for path, status, code, message in [
                        ('not-found', 404, 'not_found', 'resource not found'),
                        ('validation', 400, 'validation_error', 'request validation failed'),
                        ('internal', 500, 'internal_error', 'internal server error'),
                        ('unauthorized', 401, 'unauthorized', 'authentication required'),
                        ('basic', 401, 'unauthorized', 'authentication required'),
                        ('typed', 400, 'validation_error', 'quantity must be positive'),
                        ('slow', 504, 'request_timeout', 'request processing timed out'),
                    ]:
                        contract(request(port, '/errors/' + path), status, code, message)
                        results['checks'].append(path)
                    for path, challenge in [('unauthorized', 'Bearer'), ('basic', 'Basic realm="Orders"')]:
                        for method in ['GET', 'HEAD']:
                            actual, headers, body = request(port, '/errors/' + path, method)
                            assert actual == 401, (actual, headers, body)
                            assert dict(headers)['www-authenticate'] == challenge, headers
                            assert dict(headers)['content-type'] == 'application/json'
                            if method == 'HEAD':
                                assert body == b''
                            else:
                                assert json.loads(body) == {'code': 'unauthorized', 'message': 'authentication required'}
                    results['checks'].append('public-generated-application-authentication-GET-HEAD-challenges')
                    for body, media, status, code, message in [
                        (SENTINEL, 'application/json', 400, 'invalid_json', 'invalid JSON request'),
                        (json.dumps({'quantity': SENTINEL}), 'application/json', 422, 'validation_error', 'request validation failed'),
                        ('{}', None, 415, 'unsupported_media_type', 'content type must be application/json'),
                        (SENTINEL, 'text/plain', 415, 'unsupported_media_type', 'content type must be application/json'),
                        ('x' * (1024 * 1024 + 1), 'application/json', 413, 'payload_too_large', 'request body too large'),
                    ]:
                        headers = {'Content-Type': media} if media else {}
                        contract(request(port, '/errors/json', 'POST', body, headers), status, code, message)
                    results['checks'].append('json-400-422-415-413')
                    for path, status in [('internal', 500), ('slow', 504)]:
                        actual, headers, body = request(port, '/errors/' + path, 'HEAD')
                        assert actual == status and not body
                        assert dict(headers)['content-type'] == 'application/json'
                    status, headers, body = request(port, '/errors/typed', 'PUT', headers={'Authorization': 'Bearer issue12-token'} if secure else None)
                    assert status == 405 and 'GET' in dict(headers)['allow'] and not body, (status, headers, body)
                    status, headers, body = request(port, '/errors/native', 'POST', 'invalid', {'Content-Type': 'application/json'})
                    assert status == 400 and dict(headers)['content-type'].startswith('text/plain')
                    assert request(port, '/missing', headers={'Authorization': 'Bearer issue12-token'} if secure else None)[::2] == (404, b'')
                    results['checks'].append('HEAD-Allow-native-extractor-native-404')
                if secure and mode != 'hidden':
                    for credential in [None, 'Bearer invalid-token-sentinel', 'Basic dXNlcjp3cm9uZw==']:
                        headers = {'Authorization': credential} if credential else {}
                        response = request(port, '/protected', headers=headers)
                        contract(response, 401, 'unauthorized', 'authentication required')
                        challenges = [v for k, v in response[1] if k.lower() == 'www-authenticate']
                        assert challenges == ['Bearer', 'Basic realm="Scafra"'], challenges
                    if mode == 'normal':
                        for path in ['role', 'scope']:
                            response = request(port, '/protected/' + path, headers={'Authorization': 'Bearer issue12-token'})
                            contract(response, 403, 'forbidden', 'access denied')
                            assert 'www-authenticate' not in dict(response[1])
                    assert request(port, '/protected/public')[0] == 200
                    results['checks'].append(mode + '-authentication')
                if mode == 'hidden':
                    for credential in [None, 'Bearer issue12-token']:
                        headers = {'Authorization': credential} if credential else {}
                        hidden = request(port, '/protected/role', headers=headers)
                        unknown = request(port, '/missing', headers=headers)
                        # Date is transport metadata and can tick between calls.
                        normalize = lambda r: (r[0], [(k, v) for k, v in r[1] if k.lower() != 'date'], r[2])
                        assert normalize(hidden) == normalize(unknown), (hidden, unknown)
                        assert hidden[0] == 404 and hidden[2] == b''
                    results['checks'].append('hidden-full-HTTP1-header-equality')
                if actuator and mode == 'normal':
                    contract(request(port, '/ready'), 401, 'unauthorized', 'authentication required')
                    assert dict(request(port, '/ready')[1])['www-authenticate'] == 'Bearer'
                    status, _, body = request(port, '/ready', headers={'Authorization': 'Bearer issue12-actuator-token'})
                    assert status == 200 and json.loads(body)['status'] == 'UP'
                    results['checks'].append('independent-actuator-auth-and-health')
                child.send_signal(signal.SIGTERM)
                assert child.wait(timeout=15) == 0
                results['checks'].append(mode + '-bounded-SIGTERM')
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait(timeout=5)
        logs = logpath.read_text()
        for secret in [SENTINEL, 'invalid-token-sentinel', 'issue12-token', 'issue12-actuator-token', 'issue12-basic-password', 'issue12-signing-secret', 'dXNlcjp3cm9uZw==']:
            assert secret not in logs, f'secret in {logpath}'
    (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
