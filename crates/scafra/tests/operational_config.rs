#![cfg(feature = "web")]

use scafra::{ConfigLoader, ScafraConfig};
use std::time::Duration;

#[test]
fn validated_operational_values_are_forwarded_without_resetting_to_defaults() {
    let config = ConfigLoader::new()
        .env_prefix("CFG111_FACADE_UNUSED")
        .override_value("server.timeout", "7")
        .override_value("server.max_request_body_bytes", "1234")
        .override_value("server.header_read_timeout_seconds", "3")
        .override_value("server.idle_timeout_seconds", "5")
        .override_value("server.max_connections", "17")
        .override_value("server.max_header_bytes", "16384")
        .override_value("server.http1_keep_alive", "false")
        .override_value("server.http1_max_headers", "50")
        .override_value("server.http2_max_concurrent_streams", "80")
        .override_value("shutdown.grace_period_seconds", "9")
        .override_value("shutdown.force_after_grace", "false")
        .load_validated::<ScafraConfig>()
        .unwrap();
    let options = scafra::router_options(&config);
    assert_eq!(options.request_timeout, Some(Duration::from_secs(7)));
    assert_eq!(options.max_request_body_bytes, 1234);
    assert_eq!(
        options.connection.header_read_timeout,
        Duration::from_secs(3)
    );
    assert_eq!(options.connection.idle_timeout, Duration::from_secs(5));
    assert_eq!(options.connection.max_connections, 17);
    assert_eq!(options.connection.max_header_bytes, 16384);
    assert!(!options.connection.http1_keep_alive);
    assert_eq!(options.connection.http1_max_headers, 50);
    assert_eq!(options.connection.http2_max_concurrent_streams, 80);
    assert_eq!(
        config.shutdown.policy().grace_period(),
        Duration::from_secs(9)
    );
    assert!(!config.shutdown.policy().force_after_grace());
}

async fn request(address: std::net::SocketAddr, payload: &[u8]) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(payload).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8(response).unwrap()
}

async fn configured_server(
    config: &ScafraConfig,
    router: scafra::web::axum::Router,
) -> (
    std::net::SocketAddr,
    scafra::web::ShutdownHandle,
    tokio::task::JoinHandle<Result<scafra::web::ServerOutcome, scafra::web::ServerError>>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = scafra::web::finish_managed_router(
        scafra::web::guard_axum_router(router),
        &[],
        &scafra::router_options(config),
    )
    .unwrap();
    let (handle, shutdown) = scafra::web::shutdown_channel();
    let server = tokio::spawn(scafra::web::serve_finalized_listener_with_shutdown(
        listener,
        router,
        shutdown,
        config.shutdown.policy(),
    ));
    (address, handle, server)
}

#[tokio::test]
async fn loader_body_limit_and_processing_deadline_change_real_http_behavior() {
    use scafra::web::axum::{
        body::{to_bytes, Body},
        routing::{get, post},
        Router,
    };
    let config = ConfigLoader::new()
        .env_prefix("CFG111_HTTP_UNUSED")
        .override_value("server.max_request_body_bytes", "8")
        .override_value("server.timeout", "1")
        .load_validated::<ScafraConfig>()
        .unwrap();
    let router = Router::new()
        .route(
            "/raw",
            post(|body: Body| async move {
                match to_bytes(body, usize::MAX).await {
                    Ok(body) => body.len().to_string(),
                    Err(_) => "handler discarded body error".to_owned(),
                }
            }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                "late"
            }),
        );
    let (address, handle, server) = configured_server(&config, router).await;
    let at_limit = request(address, b"POST /raw HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 8\r\n\r\n12345678").await;
    assert!(at_limit.starts_with("HTTP/1.1 200"), "{at_limit}");
    assert!(at_limit.ends_with('8'));
    let over = request(address, b"POST /raw HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 9\r\n\r\n123456789").await;
    assert!(over.starts_with("HTTP/1.1 413"), "{over}");
    assert!(over.contains("application/json"));
    assert!(
        over.ends_with("{\"code\":\"payload_too_large\",\"message\":\"request body too large\"}")
    );
    let slow = request(
        address,
        b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(slow.starts_with("HTTP/1.1 504"), "{slow}");
    assert!(slow.contains("\"code\":\"request_timeout\""));
    handle.request().unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn loader_explicit_timeout_optout_allows_a_longer_request() {
    use scafra::web::axum::{routing::get, Router};
    let config = ConfigLoader::new()
        .env_prefix("CFG111_HTTP_DISABLED_UNUSED")
        .override_value("server.timeout", "null")
        .load_validated::<ScafraConfig>()
        .unwrap();
    let router = Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_millis(1100)).await;
            "finished"
        }),
    );
    let (address, handle, server) = configured_server(&config, router).await;
    let response = request(
        address,
        b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with("finished"));
    handle.request().unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn resolved_shutdown_grace_bounds_a_pending_http_request() {
    use scafra::web::axum::{routing::get, Router};
    use tokio::io::AsyncWriteExt;
    let config = ConfigLoader::new()
        .env_prefix("CFG111_DRAIN_UNUSED")
        .override_value("shutdown.grace_period_seconds", "1")
        .load_validated::<ScafraConfig>()
        .unwrap();
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let handler_entered = entered.clone();
    let router = Router::new().route(
        "/pending",
        get(move || {
            let entered = handler_entered.clone();
            async move {
                entered.notify_one();
                std::future::pending::<&'static str>().await
            }
        }),
    );
    let (address, handle, server) = configured_server(&config, router).await;
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(b"GET /pending HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), entered.notified())
        .await
        .unwrap();
    handle.request().unwrap();
    let error = tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    match error {
        scafra::web::ServerError::GracefulShutdownTimeout { reason, policy } => {
            assert_eq!(reason, scafra::ShutdownReason::ApplicationRequest);
            assert_eq!(policy, config.shutdown.policy());
        }
        error => panic!("expected configured drain timeout: {error}"),
    }
}
