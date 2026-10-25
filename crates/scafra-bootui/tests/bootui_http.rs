use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{Request, StatusCode},
    Router,
};
use scafra_bootui::{
    layer, try_layer_with_config, try_layer_with_config_and_profile, BootUiConfig,
    BootUiPolicyError,
};
use scafra_config::ScafraConfig;
use std::net::SocketAddr;
use tower::ServiceExt;

const BEARER_SENTINEL: &str = "bootui-bearer-secret-72";
const BASIC_USERNAME_SENTINEL: &str = "bootui-basic-user-72";
const BASIC_PASSWORD_SENTINEL: &str = "bootui-basic-password-72";
const JWT_SENTINEL: &str = "bootui-jwt-signing-secret-72";
const ACTUATOR_SENTINEL: &str = "bootui-actuator-token-72";

fn application_config(local_only: bool, host: &str) -> ScafraConfig {
    let mut config = ScafraConfig::default();
    config.bootui.enabled = true;
    config.bootui.local_only = local_only;
    config.bootui.host = host.to_owned();
    config.security.bearer_token = Some(BEARER_SENTINEL.to_owned());
    config.security.basic.username = Some(BASIC_USERNAME_SENTINEL.to_owned());
    config.security.basic.password = Some(BASIC_PASSWORD_SENTINEL.to_owned());
    config.security.jwt.enabled = true;
    config.security.jwt.secret = Some(JWT_SENTINEL.to_owned());
    config.actuator.security.bearer_token = Some(ACTUATOR_SENTINEL.to_owned());
    config
}

fn peer(address: &str) -> SocketAddr {
    match address.parse() {
        Ok(address) => address,
        Err(error) => panic!("invalid test peer address: {error}"),
    }
}

async fn request(
    router: Router,
    path: &str,
    remote: SocketAddr,
    authorization: Option<&str>,
) -> axum::response::Response {
    let mut builder = Request::builder().uri(path);
    if let Some(authorization) = authorization {
        builder = builder.header("authorization", authorization);
    }
    let mut request = match builder.body(Body::empty()) {
        Ok(request) => request,
        Err(error) => panic!("request should be valid: {error}"),
    };
    request.extensions_mut().insert(ConnectInfo(remote));
    match router.oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    }
}

async fn response_text(response: axum::response::Response) -> String {
    let body = match to_bytes(response.into_body(), 16 * 1024).await {
        Ok(body) => body,
        Err(error) => panic!("response body should be readable: {error}"),
    };
    String::from_utf8_lossy(&body).into_owned()
}

#[tokio::test]
async fn config_route_returns_only_the_safe_dashboard_snapshot_over_http() {
    let config = application_config(true, "127.0.0.1");
    let app = match try_layer_with_config(Router::new(), &config) {
        Ok(app) => app,
        Err(error) => panic!("loopback BootUI config should be accepted: {error}"),
    };

    for remote in [peer("127.0.0.1:43210"), peer("[::1]:43210")] {
        let response = request(app.clone(), "/bootui/api/config", remote, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_text(response).await;
        let value: serde_json::Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(error) => panic!("config endpoint should return valid JSON: {error}"),
        };

        assert_eq!(value["server"]["host"], "127.0.0.1");
        assert_eq!(value["server"]["port"], 8080);
        assert_eq!(value["profile"], "default");
        assert_eq!(value.as_object().map(serde_json::Map::len), Some(2));
        for sentinel in [
            BEARER_SENTINEL,
            BASIC_USERNAME_SENTINEL,
            BASIC_PASSWORD_SENTINEL,
            JWT_SENTINEL,
            ACTUATOR_SENTINEL,
        ] {
            assert!(!body.contains(sentinel), "response leaked {sentinel}");
        }
    }
}

#[tokio::test]
async fn effective_profile_is_carried_into_the_safe_snapshot() {
    let config = application_config(true, "127.0.0.1");
    let app = match try_layer_with_config_and_profile(Router::new(), &config, "production") {
        Ok(app) => app,
        Err(error) => panic!("loopback BootUI config should be accepted: {error}"),
    };
    let response = request(app, "/bootui/api/config", peer("127.0.0.1:43210"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_text(response).await;
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["profile"], "production");
    assert!(!body.contains(BEARER_SENTINEL));
    assert!(!body.contains(BASIC_PASSWORD_SENTINEL));
    assert!(!body.contains(JWT_SENTINEL));
}

#[tokio::test]
async fn existing_layer_api_still_serves_the_dashboard_with_peer_information() {
    let config = BootUiConfig {
        enabled: true,
        ..BootUiConfig::default()
    };
    let app = layer(Router::new(), &config);
    let response = request(app, "/bootui/api/config", peer("127.0.0.1:43210"), None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_text(response).await;
    let value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(error) => panic!("config endpoint should return valid JSON: {error}"),
    };
    assert!(value["server"]["port"].as_u64().is_some());
}

#[tokio::test]
async fn unknown_future_credential_fields_do_not_enter_the_http_snapshot() {
    let yaml = r#"
bootui:
  enabled: true
  host: 127.0.0.1
  local_only: true
security:
  bearer_token: future-test-bearer-72
  basic:
    username: future-test-basic-user-72
    password: future-test-basic-password-72
  jwt:
    secret: future-test-jwt-secret-72
  client_secret: future-test-client-secret-72
  signing_key: future-test-signing-key-72
  private_key: future-test-private-key-72
"#;
    let config: ScafraConfig = match serde_yaml::from_str(yaml) {
        Ok(config) => config,
        Err(error) => panic!("test configuration should deserialize: {error}"),
    };
    let app = match try_layer_with_config(Router::new(), &config) {
        Ok(app) => app,
        Err(error) => panic!("loopback BootUI config should be accepted: {error}"),
    };
    let response = request(app, "/bootui/api/config", peer("127.0.0.1:43210"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_text(response).await;

    for sentinel in [
        "future-test-bearer-72",
        "future-test-basic-user-72",
        "future-test-basic-password-72",
        "future-test-jwt-secret-72",
        "future-test-client-secret-72",
        "future-test-signing-key-72",
        "future-test-private-key-72",
    ] {
        assert!(!body.contains(sentinel), "response leaked {sentinel}");
    }
}

#[tokio::test]
async fn local_only_route_rejects_non_loopback_peers_and_keeps_dashboard_available_locally() {
    let config = application_config(true, "::1");
    let app = match try_layer_with_config(Router::new(), &config) {
        Ok(app) => app,
        Err(error) => panic!("IPv6 loopback should be accepted: {error}"),
    };

    let local = request(app.clone(), "/bootui", peer("[::1]:43210"), None).await;
    assert_eq!(local.status(), StatusCode::OK);
    assert!(response_text(local).await.contains("Scafra BootUI"));

    let remote = request(app, "/bootui/api/config", peer("192.0.2.10:43210"), None).await;
    assert_eq!(remote.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_local_policy_requires_authentication_on_the_actual_http_route() {
    let mut config = application_config(false, "0.0.0.0");
    config.security.enabled = true;
    config.security.jwt.enabled = false;
    config.security.permit_all = vec!["/bootui/api/config".to_owned()];
    let app = match try_layer_with_config(Router::new(), &config) {
        Ok(app) => app,
        Err(error) => panic!("configured authentication should protect BootUI: {error}"),
    };

    let remote = peer("192.0.2.10:43210");
    let denied = request(app.clone(), "/bootui/api/config", remote, None).await;
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let denied_body = response_text(denied).await;
    for sentinel in [BEARER_SENTINEL, BASIC_PASSWORD_SENTINEL, JWT_SENTINEL] {
        assert!(!denied_body.contains(sentinel));
    }

    let allowed = request(
        app,
        "/bootui/api/config",
        remote,
        Some(&format!("Bearer {BEARER_SENTINEL}")),
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    let allowed_body = response_text(allowed).await;
    for sentinel in [
        BEARER_SENTINEL,
        BASIC_USERNAME_SENTINEL,
        BASIC_PASSWORD_SENTINEL,
        JWT_SENTINEL,
        ACTUATOR_SENTINEL,
    ] {
        assert!(
            !allowed_body.contains(sentinel),
            "response leaked {sentinel}"
        );
    }
}

#[test]
fn local_only_rejects_non_loopback_and_invalid_host_addresses() {
    for host in ["0.0.0.0", "::", "192.0.2.8", "localhost", "not-an-address"] {
        let config = application_config(true, host);
        let result = try_layer_with_config(Router::new(), &config);
        assert!(
            matches!(
                result,
                Err(BootUiPolicyError::LoopbackRequired | BootUiPolicyError::InvalidHost)
            ),
            "host {host} should not produce a BootUI route"
        );
    }
}

#[test]
fn local_only_false_fails_closed_without_explicit_valid_authentication() {
    let config = application_config(false, "0.0.0.0");
    assert!(matches!(
        try_layer_with_config(Router::new(), &config),
        Err(BootUiPolicyError::AuthenticationRequired)
    ));

    let mut invalid_security = config;
    invalid_security.security.enabled = true;
    invalid_security.security.bearer_token = None;
    invalid_security.security.basic.username = None;
    invalid_security.security.basic.password = None;
    invalid_security.security.jwt.enabled = false;
    assert!(matches!(
        try_layer_with_config(Router::new(), &invalid_security),
        Err(BootUiPolicyError::AuthenticationRequired)
            | Err(BootUiPolicyError::InvalidSecurityConfiguration)
    ));
}

#[test]
fn invalid_dashboard_paths_return_errors_instead_of_panicking() {
    for path in [
        "/",
        "/{",
        "/{*rest}",
        "/:legacy",
        "/a?secret",
        "/a#secret",
        "/a\\b",
        "/a\nsecret",
    ] {
        let mut config = scafra_config::ScafraConfig::default();
        config.bootui.enabled = true;
        config.bootui.path = path.to_owned();
        assert!(
            matches!(
                scafra_bootui::try_layer_with_config(axum::Router::new(), &config),
                Err(scafra_bootui::BootUiPolicyError::InvalidPath)
            ),
            "invalid path must be rejected"
        );
    }
}

#[tokio::test]
async fn owned_dashboard_uses_effective_resource_limits_and_is_joined_on_shutdown() {
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    let reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let mut config = scafra_config::ScafraConfig::default();
    config.bootui.enabled = true;
    config.bootui.port = address.port();
    config.server.max_request_body_bytes = 8;
    config.server.header_read_timeout_seconds = 1;
    config.server.idle_timeout_seconds = 1;
    let owner = scafra_bootui::start_managed_with_config(&config, "effective-profile")
        .unwrap()
        .unwrap();
    let mut client = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(client) = TcpStream::connect(address).await {
                break client;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    client.write_all(b"GET /bootui/config HTTP/1.1\r\nHost: localhost\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789").await.unwrap();
    let mut bytes = Vec::new();
    client.read_to_end(&mut bytes).await.unwrap();
    let response = String::from_utf8(bytes).unwrap();
    assert!(response.starts_with("HTTP/1.1 413"));
    assert!(response.contains("payload_too_large"));
    let mut client = TcpStream::connect(address).await.unwrap();
    client
        .write_all(b"GET /bootui HTTP/1.1\r\nHost: incomplete")
        .await
        .unwrap();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), client.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert!(bytes.is_empty() || String::from_utf8_lossy(&bytes).starts_with("HTTP/1.1 408"));
    owner.shutdown().await.unwrap();
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn managed_nonlocal_dashboard_cannot_inherit_application_permit_all() {
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    let reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let mut config = scafra_config::ScafraConfig::default();
    config.bootui.enabled = true;
    config.bootui.port = address.port();
    config.bootui.local_only = false;
    config.security.enabled = true;
    config.security.bearer_token = Some("managed-ui-token".into());
    config.security.permit_all = vec![
        "/bootui/api/config".into(),
        "/api/config".into(),
        "/config".into(),
    ];
    let owner = scafra_bootui::start_managed_with_config(&config, "effective-profile")
        .unwrap()
        .unwrap();
    for token in [None, Some("wrong-token"), Some("managed-ui-token")] {
        let mut client = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(client) = TcpStream::connect(address).await {
                    break client;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        client.write_all(format!("GET /bootui/api/config HTTP/1.1\r\nHost: localhost\r\n{authorization}Connection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        client.read_to_end(&mut bytes).await.unwrap();
        let response = String::from_utf8(bytes).unwrap();
        if token == Some("managed-ui-token") {
            assert!(response.starts_with("HTTP/1.1 200"), "{response}");
            assert!(!response.contains("managed-ui-token"));
        } else {
            assert!(response.starts_with("HTTP/1.1 401"), "{response}");
        }
    }
    owner.shutdown().await.unwrap();
}
