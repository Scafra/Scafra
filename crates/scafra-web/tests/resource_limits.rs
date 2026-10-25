use axum::{
    body::{to_bytes, Body, Bytes},
    extract::{ConnectInfo, Request},
    http::{Request as HttpRequest, StatusCode},
    routing::{get, post},
    Router,
};
use http_body::{Body as HttpBody, Frame};
use http_body_util::BodyExt;
use scafra_foundation::{ShutdownPolicy, ShutdownReason};
use scafra_web::{
    finish_managed_router, serve_finalized_listener_with_shutdown, with_request_body_limit,
    with_request_timeout, RouterOptions,
};
use std::{
    collections::VecDeque,
    convert::Infallible,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tower::ServiceExt;

struct Chunks(VecDeque<Result<Frame<Bytes>, std::io::Error>>);
impl HttpBody for Chunks {
    type Data = Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        Poll::Ready(self.0.pop_front())
    }
}
fn chunks(parts: &[&'static str]) -> Body {
    Body::new(Chunks(
        parts
            .iter()
            .map(|part| Ok(Frame::data(Bytes::from_static(part.as_bytes()))))
            .collect(),
    ))
}
async fn consume(mut request: Request) -> String {
    let mut bytes = 0;
    // Deliberately swallow stream errors: framework must still send 413.
    while let Some(Ok(frame)) = request.body_mut().frame().await {
        if let Some(data) = frame.data_ref() {
            bytes += data.len();
        }
    }
    bytes.to_string()
}
fn routes() -> Router {
    Router::new()
        .route("/raw", post(consume))
        .route("/ignore", post(|| async { "ignored" }))
        .route(
            "/stop",
            post(|mut request: Request| async move {
                let _ = request.body_mut().frame().await;
                "stopped"
            }),
        )
        .route(
            "/echo",
            post(|request: Request| async { request.into_body() }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(80)).await;
                "done"
            }),
        )
        .route(
            "/json",
            post(
                |scafra_web::JsonBody(value): scafra_web::JsonBody<Vec<String>>| async move {
                    value.len().to_string()
                },
            ),
        )
        .route(
            "/peer",
            get(
                |ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>| async move {
                    peer.ip().to_string()
                },
            ),
        )
}
fn bounded(router: Router, limit: usize) -> Router {
    finish_managed_router(
        router,
        &[],
        &RouterOptions {
            max_request_body_bytes: limit,
            ..Default::default()
        },
    )
    .unwrap()
    .into_router()
}
async fn send(
    router: Router,
    path: &str,
    body: Body,
    declared: Option<&str>,
) -> axum::response::Response {
    let mut request = HttpRequest::post(path).header("content-type", "application/json");
    if let Some(length) = declared {
        request = request.header("content-length", length);
    }
    router.oneshot(request.body(body).unwrap()).await.unwrap()
}
async fn assert_413(response: axum::response::Response) {
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        r#"{"code":"payload_too_large","message":"request body too large"}"#
    );
}
#[tokio::test]
async fn actual_bytes_enforced_even_with_unknown_or_misleading_lengths_and_swallowed_errors() {
    for (parts, status) in [
        (vec!["123", "456"], StatusCode::OK),
        (vec!["1234", "5678"], StatusCode::OK),
        (vec!["1234", "5678", "9"], StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        for declared in [None, Some("1")] {
            let response = send(bounded(routes(), 8), "/raw", chunks(&parts), declared).await;
            if status == StatusCode::PAYLOAD_TOO_LARGE {
                assert_413(response).await;
            } else {
                assert_eq!(response.status(), status);
            }
        }
    }
    for len in [7, 8, 9] {
        let response = send(
            bounded(routes(), 8),
            "/raw",
            Body::from("x".repeat(len)),
            None,
        )
        .await;
        if len > 8 {
            assert_413(response).await;
        } else {
            assert_eq!(response.status(), StatusCode::OK);
        }
    }
}
#[tokio::test]
async fn json_and_large_raw_bodies_use_default_and_custom_ceiling() {
    for path in ["/json", "/raw"] {
        let value = format!("[\"{}\"]", "x".repeat(1_048_576));
        assert_413(send(bounded(routes(), 1_048_576), path, Body::from(value), None).await).await;
    }
    let value = "[\"123456\"]";
    assert_413(send(bounded(routes(), 8), "/json", Body::from(value), None).await).await;
    assert_eq!(
        send(bounded(routes(), 16), "/json", Body::from(value), None)
            .await
            .status(),
        StatusCode::OK
    );
}
#[tokio::test]
async fn unread_streams_and_response_commit_have_explicit_boundaries() {
    for path in ["/ignore", "/stop"] {
        let response = send(
            bounded(routes(), 8),
            path,
            chunks(&["1234", "5678", "9"]),
            None,
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "unread bytes cannot be classified without draining"
        );
        assert_413(send(bounded(routes(), 8), path, Body::from("123456789"), None).await).await;
    }
    let response = send(
        bounded(routes(), 8),
        "/echo",
        chunks(&["1234", "5678", "9"]),
        None,
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "response headers precede body polling"
    );
    assert!(
        to_bytes(response.into_body(), 1024).await.is_err(),
        "stream fails instead of forwarding excess bytes"
    );
}
#[tokio::test]
async fn route_layers_tighten_and_cannot_raise_global_ceiling() {
    let local = with_request_body_limit(routes(), 4).unwrap();
    assert_413(send(bounded(local, 8), "/raw", chunks(&["123", "45"]), None).await).await;
    let local = with_request_body_limit(routes(), 16).unwrap();
    assert_413(send(bounded(local, 8), "/raw", chunks(&["12345678", "9"]), None).await).await;
    assert!(with_request_body_limit(routes(), 0).is_err());
}

async fn start(
    options: RouterOptions,
    router: Router,
) -> (
    std::net::SocketAddr,
    oneshot::Sender<ShutdownReason>,
    tokio::task::JoinHandle<Result<scafra_web::ServerOutcome, scafra_web::ServerError>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = finish_managed_router(router, &[], &options).unwrap();
    let (tx, rx) = oneshot::channel();
    let server = tokio::spawn(serve_finalized_listener_with_shutdown(
        listener,
        router,
        async { rx.await.unwrap_or(ShutdownReason::RuntimeFailure) },
        ShutdownPolicy::new(Duration::from_secs(1), true),
    ));
    (address, tx, server)
}
async fn tcp(address: std::net::SocketAddr, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(request).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8(response).unwrap()
}
#[tokio::test]
async fn chunked_and_length_framed_requests_enforced_on_the_wire() {
    let (address, tx, server) = start(
        RouterOptions {
            max_request_body_bytes: 8,
            ..Default::default()
        },
        routes(),
    )
    .await;
    for body in [
        "3\r\n123\r\n4\r\n4567\r\n0\r\n\r\n",
        "4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n",
        "4\r\n1234\r\n5\r\n56789\r\n0\r\n\r\n",
    ] {
        let request=format!("POST /raw HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n{body}");
        let response = tcp(address, request.as_bytes()).await;
        if body.contains("56789") {
            assert!(response.starts_with("HTTP/1.1 413"));
            assert!(response.contains("payload_too_large"));
        } else {
            assert!(response.starts_with("HTTP/1.1 200"));
        }
    }
    let response=tcp(address,b"POST /raw HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 9\r\n\r\n123456789").await;
    assert!(response.starts_with("HTTP/1.1 413"));
    let response = tcp(
        address,
        b"POST /raw HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 200"));
    // Hyper owns framing: bytes beyond declared length are not the same body.
    let response=tcp(address,b"POST /raw HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 1\r\n\r\n123456789").await;
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.ends_with('1'));
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn processing_deadlines_and_route_tightening_use_real_http() {
    for timeout in [
        Some(Duration::from_millis(10)),
        Some(Duration::from_secs(1)),
        None,
    ] {
        let (address, tx, server) = start(
            RouterOptions {
                request_timeout: timeout,
                ..Default::default()
            },
            routes(),
        )
        .await;
        let response = tcp(
            address,
            b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        if timeout == Some(Duration::from_millis(10)) {
            assert!(response.starts_with("HTTP/1.1 504"));
            assert!(response.contains("request_timeout"));
        } else {
            assert!(response.starts_with("HTTP/1.1 200"));
        }
        tx.send(ShutdownReason::ApplicationRequest).unwrap();
        server.await.unwrap().unwrap();
    }
    let router = with_request_timeout(routes(), Some(Duration::from_millis(5))).unwrap();
    let (address, tx, server) = start(RouterOptions::default(), router).await;
    assert!(tcp(
        address,
        b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .await
    .starts_with("HTTP/1.1 504"));
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn request_future_is_cancelled_and_deadline_wins_ties() {
    struct DropSignal(Arc<AtomicBool>);
    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let observed = dropped.clone();
    let router = Router::new().route(
        "/",
        get(move || {
            let signal = DropSignal(observed.clone());
            async move {
                let _signal = signal;
                std::future::pending::<Infallible>().await
            }
        }),
    );
    let router = finish_managed_router(
        router,
        &[],
        &RouterOptions {
            request_timeout: Some(Duration::from_millis(5)),
            ..Default::default()
        },
    )
    .unwrap()
    .into_router();
    assert_eq!(
        router
            .oneshot(HttpRequest::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::GATEWAY_TIMEOUT
    );
    assert!(dropped.load(Ordering::SeqCst));
}
#[tokio::test]
async fn slow_headers_idle_and_keepalive_controls_survive_finalization() {
    let mut options = RouterOptions::default();
    options.connection.header_read_timeout = Duration::from_millis(50);
    options.connection.idle_timeout = Duration::from_millis(120);
    let (address, tx, server) = start(options, routes()).await;
    let response = tcp(address, b"GET /peer HTTP/1.1\r\nHost: local").await;
    assert!(response.is_empty() || response.starts_with("HTTP/1.1 408"));
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(b"GET /peer HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    let response = String::from_utf8(response).unwrap();
    assert!(response.contains("127.0.0.1"));
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
    let mut options = RouterOptions::default();
    options.connection.http1_keep_alive = false;
    let (address, tx, server) = start(options, routes()).await;
    assert!(
        tcp(address, b"GET /peer HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .starts_with("HTTP/1.1 200")
    );
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn http2_and_peer_information_are_preserved() {
    let (address, tx, server) = start(RouterOptions::default(), routes()).await;
    let stream = TcpStream::connect(address).await.unwrap();
    let (mut sender, connection) = hyper::client::conn::http2::handshake(
        hyper_util::rt::TokioExecutor::new(),
        hyper_util::rt::TokioIo::new(stream),
    )
    .await
    .unwrap();
    let client = tokio::spawn(connection);
    let response = sender
        .send_request(
            HttpRequest::get(format!("http://{address}/peer"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.version(), axum::http::Version::HTTP_2);
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "127.0.0.1"
    );
    drop(sender);
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
    client.await.unwrap().unwrap();
}

#[tokio::test]
async fn admission_ceiling_defers_next_connection_until_slot_is_released() {
    let mut options = RouterOptions::default();
    options.connection.max_connections = 1;
    let (address, tx, server) = start(options, routes()).await;
    let mut occupied = TcpStream::connect(address).await.unwrap();
    occupied
        .write_all(b"GET /peer HTTP/1.1\r\nHost: local")
        .await
        .unwrap();
    // Ensure this connection has entered the sole admitted slot.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let mut waiting = TcpStream::connect(address).await.unwrap();
    waiting
        .write_all(b"GET /peer HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut bytes = Vec::new();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), waiting.read_to_end(&mut bytes))
            .await
            .is_err()
    );
    drop(occupied);
    tokio::time::timeout(Duration::from_secs(2), waiting.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert!(String::from_utf8(bytes)
        .unwrap()
        .starts_with("HTTP/1.1 200"));
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn header_buffer_ceiling_and_body_source_failure_remain_bounded() {
    let mut options = RouterOptions::default();
    options.connection.max_header_bytes = 8192;
    let (address, tx, server) = start(options, routes()).await;
    let response = tcp(
        address,
        format!(
            "GET /peer HTTP/1.1\r\nHost: localhost\r\nX-Large: {}\r\nConnection: close\r\n\r\n",
            "x".repeat(20000)
        )
        .as_bytes(),
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 431"));
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
    let body = Body::new(Chunks(VecDeque::from([
        Ok(Frame::data(Bytes::from_static(b"1234"))),
        Err(std::io::Error::other("source-secret")),
    ])));
    let response = send(bounded(routes(), 8), "/json", body, None).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        !String::from_utf8_lossy(&to_bytes(response.into_body(), 1024).await.unwrap())
            .contains("source-secret")
    );
}

#[tokio::test]
async fn default_processing_deadline_is_enforced_over_tcp() {
    let router = Router::new().route(
        "/pending",
        get(|| async { std::future::pending::<&'static str>().await }),
    );
    let (address, tx, server) = start(RouterOptions::default(), router).await;
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(b"GET /pending HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    let start = std::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(35), stream.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(start.elapsed() >= Duration::from_secs(29));
    let response = String::from_utf8(response).unwrap();
    assert!(response.starts_with("HTTP/1.1 504"));
    assert!(response.contains("request_timeout"));
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn exceeded_body_is_dropped_and_fused_even_when_handler_continues_after_errors() {
    use std::sync::atomic::AtomicUsize;
    struct Counted {
        polls: Arc<AtomicUsize>,
        dropped: Arc<AtomicBool>,
    }
    impl HttpBody for Counted {
        type Data = Bytes;
        type Error = std::io::Error;
        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
            self.polls.fetch_add(1, Ordering::SeqCst);
            Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"12345")))))
        }
    }
    impl Drop for Counted {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }
    let polls = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    let body = Body::new(Counted {
        polls: polls.clone(),
        dropped: dropped.clone(),
    });
    let router = Router::new().route(
        "/raw",
        post(|mut request: Request| async move {
            while request.body_mut().frame().await.is_some() {}
            "swallowed"
        }),
    );
    assert_413(send(bounded(router, 8), "/raw", body, None).await).await;
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn connection_inactivity_deadline_cancels_a_handler_even_with_processing_disabled() {
    let mut options = RouterOptions {
        request_timeout: None,
        ..Default::default()
    };
    options.connection.idle_timeout = Duration::from_millis(40);
    let router = Router::new().route(
        "/pending",
        get(|| async { std::future::pending::<&'static str>().await }),
    );
    let (address, tx, server) = start(options, router).await;
    assert!(
        tcp(address, b"GET /pending HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .is_empty()
    );
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn configured_header_count_and_http2_stream_concurrency_are_effective() {
    let mut options = RouterOptions::default();
    options.connection.http1_max_headers = 2;
    let (address, tx, server) = start(options, routes()).await;
    assert!(tcp(
        address,
        b"GET /peer HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nX-Extra: third\r\n\r\n"
    )
    .await
    .starts_with("HTTP/1.1 431"));
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
    use std::sync::atomic::AtomicUsize;
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(tokio::sync::Notify::new());
    let entered = Arc::new(tokio::sync::Notify::new());
    let (observed, unblock, started) = (calls.clone(), release.clone(), entered.clone());
    let router = Router::new().route(
        "/hold",
        get(move || {
            let (calls, release, entered) = (observed.clone(), unblock.clone(), started.clone());
            async move {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    entered.notify_one();
                    release.notified().await;
                }
                "done"
            }
        }),
    );
    let mut options = RouterOptions::default();
    options.connection.http2_max_concurrent_streams = 1;
    let (address, tx, server) = start(options, router).await;
    let stream = TcpStream::connect(address).await.unwrap();
    let (mut sender, connection) = hyper::client::conn::http2::handshake(
        hyper_util::rt::TokioExecutor::new(),
        hyper_util::rt::TokioIo::new(stream),
    )
    .await
    .unwrap();
    let client = tokio::spawn(connection);
    let mut another = sender.clone();
    let first = tokio::spawn(async move {
        sender
            .send_request(
                HttpRequest::get(format!("http://{address}/hold"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_body()
            .collect()
            .await
            .unwrap()
    });
    entered.notified().await;
    let second = tokio::spawn(async move {
        another
            .send_request(
                HttpRequest::get(format!("http://{address}/hold"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_body()
            .collect()
            .await
            .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    release.notify_one();
    first.await.unwrap();
    second.await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
    client.await.unwrap().unwrap();
}

#[tokio::test]
async fn explicit_nonforcing_policy_waits_beyond_grace_for_owned_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let started = entered.clone();
    let router = Router::new().route(
        "/",
        get(move || {
            let entered = started.clone();
            async move {
                entered.notify_one();
                tokio::time::sleep(Duration::from_millis(50)).await;
                "drained"
            }
        }),
    );
    let router = finish_managed_router(router, &[], &RouterOptions::default()).unwrap();
    let (tx, rx) = oneshot::channel();
    let server = tokio::spawn(serve_finalized_listener_with_shutdown(
        listener,
        router,
        async { rx.await.unwrap() },
        ShutdownPolicy::new(Duration::from_millis(1), false),
    ));
    let request = tokio::spawn(async move {
        tcp(
            address,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await
    });
    entered.notified().await;
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    assert!(request.await.unwrap().ends_with("drained"));
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn upgraded_sockets_retain_admission_capacity_until_application_releases_io() {
    let release = Arc::new(tokio::sync::Notify::new());
    let ready = Arc::new(tokio::sync::Notify::new());
    let (unblock, upgraded) = (release.clone(), ready.clone());
    let router = Router::new()
        .route(
            "/upgrade",
            get(move |mut request: Request| {
                let (release, ready) = (unblock.clone(), upgraded.clone());
                async move {
                    let upgrade = hyper::upgrade::on(&mut request);
                    tokio::spawn(async move {
                        let io = upgrade.await.unwrap();
                        ready.notify_one();
                        release.notified().await;
                        drop(io);
                    });
                    axum::response::Response::builder()
                        .status(StatusCode::SWITCHING_PROTOCOLS)
                        .header("connection", "upgrade")
                        .header("upgrade", "test")
                        .body(Body::empty())
                        .unwrap()
                }
            }),
        )
        .route("/", get(|| async { "admitted" }));
    let mut options = RouterOptions::default();
    options.connection.max_connections = 1;
    let (address, tx, server) = start(options, router).await;
    let mut first = TcpStream::connect(address).await.unwrap();
    first.write_all(b"GET /upgrade HTTP/1.1\r\nHost: localhost\r\nConnection: upgrade\r\nUpgrade: test\r\n\r\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), ready.notified())
        .await
        .unwrap();
    let mut response = vec![0; 1024];
    let length = first.read(&mut response).await.unwrap();
    assert!(response[..length].starts_with(b"HTTP/1.1 101"));
    let mut waiting = TcpStream::connect(address).await.unwrap();
    waiting
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut bytes = Vec::new();
    assert!(
        tokio::time::timeout(Duration::from_millis(40), waiting.read_to_end(&mut bytes))
            .await
            .is_err()
    );
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), waiting.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert!(String::from_utf8(bytes).unwrap().ends_with("admitted"));
    drop(first);
    tx.send(ShutdownReason::ApplicationRequest).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forced_http2_drain_cancels_pending_handler_before_returning() {
    for _ in 0..32 {
        struct DropSignal(Arc<AtomicBool>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(tokio::sync::Notify::new());
        let (started, observed) = (entered.clone(), dropped.clone());
        let router = Router::new().route(
            "/pending",
            get(move || {
                let (signal, entered) = (DropSignal(observed.clone()), started.clone());
                async move {
                    let _signal = signal;
                    entered.notify_one();
                    std::future::pending::<&'static str>().await
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = finish_managed_router(
            router,
            &[],
            &RouterOptions {
                request_timeout: None,
                ..Default::default()
            },
        )
        .unwrap();
        let (tx, rx) = oneshot::channel();
        let server = tokio::spawn(serve_finalized_listener_with_shutdown(
            listener,
            router,
            async { rx.await.unwrap() },
            ShutdownPolicy::new(Duration::from_millis(10), true),
        ));
        let stream = TcpStream::connect(address).await.unwrap();
        let (mut sender, connection) = hyper::client::conn::http2::handshake(
            hyper_util::rt::TokioExecutor::new(),
            hyper_util::rt::TokioIo::new(stream),
        )
        .await
        .unwrap();
        let client = tokio::spawn(connection);
        let request = tokio::spawn(async move {
            sender
                .send_request(
                    HttpRequest::get(format!("http://{address}/pending"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
        });
        entered.notified().await;
        tx.send(ShutdownReason::ApplicationRequest).unwrap();
        assert!(matches!(
            server.await.unwrap(),
            Err(scafra_web::ServerError::GracefulShutdownTimeout { .. })
        ));
        assert!(
            dropped.load(Ordering::SeqCst),
            "handler must be cancelled before resources can be released"
        );
        assert!(request.await.unwrap().is_err());
        let _ = client.await;
    }
}
