#![cfg(feature = "web")]

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use scafra::prelude::*;
use serde::Deserialize;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tower::{service_fn, ServiceExt};

const SECRET: &str = "issue12-secret-token-password-database://private/path";

// An application-owned error type needs only Axum's existing trait.
struct DomainError;
impl IntoResponse for DomainError {
    fn into_response(self) -> Response {
        AppError::public_validation("quantity must be positive").into_response()
    }
}

#[derive(Deserialize)]
struct Input {
    _quantity: u32,
}

#[controller("/errors", public)]
struct Errors;

#[routes(default)]
impl Errors {
    #[get("/{case}")]
    async fn application(&self, case: Path<String>) -> scafra::Result<&'static str> {
        Err(match case.0.as_str() {
            "absent" => AppError::NotFound,
            "validation" => AppError::Validation(SECRET.into()),
            "unauthorized" => AppError::Unauthorized,
            "forbidden" => AppError::Forbidden,
            _ => AppError::internal(io::Error::other(SECRET)),
        })
    }

    #[get("/typed")]
    async fn typed(&self) -> std::result::Result<&'static str, DomainError> {
        Err(DomainError)
    }

    #[post("/json")]
    async fn json(&self, _body: JsonBody<Input>) -> &'static str {
        "ok"
    }

    #[post("/native")]
    async fn native(&self, _body: Json<Input>) -> &'static str {
        "ok"
    }

    #[get("/slow")]
    async fn slow(&self) -> &'static str {
        std::future::pending().await
    }
}

fn app(timeout: Option<Duration>) -> axum::Router {
    let _ = captured_logs();
    scafra::web::finish_managed_router(
        Errors::register_routes(axum::Router::new()),
        Errors::route_metadata(),
        &scafra::web::RouterOptions {
            request_timeout: timeout,
            ..Default::default()
        },
    )
    .unwrap()
    .into_router()
}

async fn assert_error(response: Response, status: StatusCode, code: &str, message: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = to_bytes(response.into_body(), 2048).await.unwrap();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(!text.contains(SECRET));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"code": code, "message": message})
    );
}

#[tokio::test]
async fn generated_controllers_return_typed_redacted_errors_and_timeout_payloads() {
    for (case, status, code, message) in [
        ("absent", 404, "not_found", "resource not found"),
        (
            "validation",
            400,
            "validation_error",
            "request validation failed",
        ),
        (
            "unauthorized",
            401,
            "unauthorized",
            "authentication required",
        ),
        ("forbidden", 403, "forbidden", "access denied"),
        ("internal", 500, "internal_error", "internal server error"),
        (
            "typed",
            400,
            "validation_error",
            "quantity must be positive",
        ),
        (
            "slow",
            504,
            "request_timeout",
            "request processing timed out",
        ),
    ] {
        let response = app(Some(Duration::from_millis(10)))
            .oneshot(
                Request::get(format!("/errors/{case}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_error(
            response,
            StatusCode::from_u16(status).unwrap(),
            code,
            message,
        )
        .await;
    }
}

#[derive(Clone, Default)]
struct LogWriter(Arc<Mutex<Vec<u8>>>);
impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// One subscriber for this integration-test process avoids tracing callsite
// interest races between concurrent tests with and without local subscribers.
fn captured_logs() -> LogWriter {
    static OUTPUT: OnceLock<LogWriter> = OnceLock::new();
    OUTPUT
        .get_or_init(|| {
            let output = LogWriter::default();
            let writer = output.clone();
            let subscriber = tracing_subscriber::fmt()
                .with_ansi(false)
                .with_max_level(tracing::Level::TRACE)
                .with_writer(move || writer.clone())
                .finish();
            tracing::subscriber::set_global_default(subscriber).unwrap();
            output
        })
        .clone()
}

#[tokio::test]
async fn parser_stream_validation_and_internal_secrets_do_not_enter_bodies_or_logs() {
    let output = captured_logs();
    async {
        let cases = [
            (
                Some("application/json"),
                format!("{SECRET} trailing"),
                400,
                "invalid_json",
                "invalid JSON request",
            ),
            (
                Some("application/json"),
                format!(r#"{{"_quantity":"{SECRET}"}}"#),
                422,
                "validation_error",
                "request validation failed",
            ),
            (
                None,
                SECRET.into(),
                415,
                "unsupported_media_type",
                "content type must be application/json",
            ),
            (
                Some("text/plain"),
                SECRET.into(),
                415,
                "unsupported_media_type",
                "content type must be application/json",
            ),
            (
                Some("application/json"),
                SECRET.repeat(32_000),
                413,
                "payload_too_large",
                "request body too large",
            ),
        ];
        for (media, body, status, code, message) in cases {
            let mut request = Request::post("/errors/json").header("x-secret", SECRET);
            if let Some(media) = media {
                request = request.header("content-type", media);
            }
            let response = app(None)
                .oneshot(request.body(Body::from(body)).unwrap())
                .await
                .unwrap();
            assert_error(
                response,
                StatusCode::from_u16(status).unwrap(),
                code,
                message,
            )
            .await;
        }
        let stream = service_fn(|_| async { Err::<Vec<u8>, _>(io::Error::other(SECRET)) })
            .call_all(Body::from("trigger").into_data_stream());
        let response = app(None)
            .oneshot(
                Request::post("/errors/json")
                    .header("content-type", "application/json")
                    .body(Body::from_stream(stream))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_error(
            response,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "internal server error",
        )
        .await;
        for case in ["validation", "internal"] {
            let response = app(None)
                .oneshot(
                    Request::get(format!("/errors/{case}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            assert!(!std::str::from_utf8(&body).unwrap().contains(SECRET));
        }
    }
    .await;
    let logs = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains("request failed"),
        "capture must observe internal diagnostics"
    );
    assert!(logs.contains("request body could not be read"));
    assert!(!logs.contains(SECRET), "secret in logs: {logs}");
}

#[tokio::test]
async fn native_axum_rejections_fallbacks_and_protocol_headers_remain_unchanged() {
    let response = app(None)
        .oneshot(
            Request::post("/errors/native")
                .header("content-type", "application/json")
                .body(Body::from("invalid"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
    let unknown = app(None)
        .oneshot(Request::get("/unknown").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    assert!(to_bytes(unknown.into_body(), 1024)
        .await
        .unwrap()
        .is_empty());
    let method = app(None)
        .oneshot(Request::put("/errors/typed").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(method.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(method.headers()["allow"].to_str().unwrap().contains("GET"));
    for path in ["/errors/internal", "/errors/slow"] {
        let response = app(Some(Duration::from_millis(10)))
            .oneshot(
                Request::builder()
                    .method(Method::HEAD)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if path.ends_with("slow") {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        );
        assert_eq!(response.headers()["content-type"], "application/json");
        assert!(to_bytes(response.into_body(), 1024)
            .await
            .unwrap()
            .is_empty());
    }
    let router = scafra::web::finish_managed_router(
        axum::Router::new().fallback(|| async { (StatusCode::NOT_FOUND, "custom missing") }),
        &[],
        &Default::default(),
    )
    .unwrap()
    .into_router();
    let response = router
        .oneshot(Request::get("/unknown").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        "custom missing"
    );
}

#[cfg(feature = "security")]
mod security {
    use super::*;
    use scafra::{BasicAuthConfig, SecurityConfig};

    #[controller("/protected", authenticated)]
    #[derive(Default)]
    struct Protected;
    #[routes]
    impl Protected {
        #[get("/")]
        async fn get(&self) -> &'static str {
            "authenticated"
        }
        #[get("/role")]
        #[roles("admin")]
        async fn role(&self) -> &'static str {
            "admin"
        }
        #[get("/scope")]
        #[scopes("data.read")]
        async fn scope(&self) -> &'static str {
            "scoped"
        }
        #[get("/open")]
        #[public]
        async fn open(&self) -> &'static str {
            "public"
        }
    }

    fn guarded(config: SecurityConfig) -> axum::Router {
        let _ = captured_logs();
        let router =
            axum::Router::new().nest("/v2", Protected::register_routes(axum::Router::new()));
        scafra::web::finish_managed_router(
            router,
            Protected::route_metadata(),
            &scafra::web::RouterOptions {
                security: config,
                ..Default::default()
            },
        )
        .unwrap()
        .into_router()
    }

    fn config() -> SecurityConfig {
        SecurityConfig {
            enabled: true,
            bearer_token: Some(SECRET.into()),
            basic: BasicAuthConfig {
                username: Some("user".into()),
                password: Some("password-sentinel".into()),
            },
            ..Default::default()
        }
    }

    async fn send(router: axum::Router, path: &str, credentials: Option<&str>) -> Response {
        let mut request = Request::get(path);
        if let Some(value) = credentials {
            request = request.header("authorization", value);
        }
        router
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn nested_managed_denials_keep_challenges_and_do_not_log_credentials() {
        let output = captured_logs();
        async {
            for value in [
                None,
                Some("Basic dXNlcjpwYXNzd29yZC1zZW50aW5lbC13cm9uZw=="),
                Some("Bearer invalid-token-sentinel"),
            ] {
                let response = send(guarded(config()), "/v2/protected", value).await;
                let challenges: Vec<_> = response
                    .headers()
                    .get_all("www-authenticate")
                    .iter()
                    .map(|h| h.to_str().unwrap())
                    .collect();
                assert_eq!(challenges, ["Bearer", "Basic realm=\"Scafra\""]);
                assert_error(
                    response,
                    StatusCode::UNAUTHORIZED,
                    "unauthorized",
                    "authentication required",
                )
                .await;
            }
            let bearer = format!("Bearer {SECRET}");
            for path in ["/v2/protected/role", "/v2/protected/scope"] {
                let response = send(guarded(config()), path, Some(&bearer)).await;
                assert!(!response.headers().contains_key("www-authenticate"));
                assert_error(
                    response,
                    StatusCode::FORBIDDEN,
                    "forbidden",
                    "access denied",
                )
                .await;
            }
            assert_eq!(
                send(guarded(config()), "/v2/protected/open", None)
                    .await
                    .status(),
                StatusCode::OK
            );
            // Hide both authentication and authorization failures exactly like
            // the native unknown-route response, including headers and body.
            for credentials in [None, Some(bearer.as_str())] {
                let hidden_config = SecurityConfig {
                    hide_unauthorized: true,
                    ..config()
                };
                let response = send(
                    guarded(hidden_config.clone()),
                    "/v2/protected/role",
                    credentials,
                )
                .await;
                let missing = send(guarded(hidden_config), "/missing", credentials).await;
                assert_eq!(response.status(), StatusCode::NOT_FOUND);
                assert_eq!(missing.status(), StatusCode::NOT_FOUND);
                // Axum adds Content-Length at different stages for selected
                // routes and its default fallback. Hyper normalizes empty
                // responses on the wire; compare privacy-bearing headers here.
                for header in ["content-type", "www-authenticate", "allow"] {
                    assert_eq!(
                        response.headers().get(header),
                        missing.headers().get(header)
                    );
                }
                assert!(to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap()
                    .is_empty());
                assert!(to_bytes(missing.into_body(), 1024)
                    .await
                    .unwrap()
                    .is_empty());
            }
            let response = guarded(config())
                .oneshot(
                    Request::builder()
                        .method(Method::HEAD)
                        .uri("/v2/protected")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert!(response.headers().contains_key("www-authenticate"));
            assert!(to_bytes(response.into_body(), 1024)
                .await
                .unwrap()
                .is_empty());
        }
        .await;
        let logs = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
        for secret in [
            SECRET,
            "password-sentinel",
            "invalid-token-sentinel",
            "dXNlcjpwYXNzd29yZC1zZW50aW5lbC13cm9uZw==",
        ] {
            assert!(!logs.contains(secret), "credential leaked in logs: {logs}");
        }
    }

    #[tokio::test]
    async fn unconfigured_authentication_schemes_are_not_advertised() {
        for (config, challenge) in [
            (
                SecurityConfig {
                    basic: Default::default(),
                    ..config()
                },
                "Bearer",
            ),
            (
                SecurityConfig {
                    bearer_token: None,
                    ..config()
                },
                "Basic realm=\"Scafra\"",
            ),
        ] {
            let response = send(guarded(config), "/v2/protected", None).await;
            assert_eq!(response.headers()["www-authenticate"], challenge);
            assert_eq!(
                response
                    .headers()
                    .get_all("www-authenticate")
                    .iter()
                    .count(),
                1
            );
        }
    }

    #[cfg(feature = "jwt")]
    #[tokio::test]
    async fn invalid_jwt_is_a_redacted_challenged_401() {
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &serde_json::json!({"sub": SECRET, "exp": 4_000_000_000u64}),
            &jsonwebtoken::EncodingKey::from_secret(b"wrong-signing-secret"),
        )
        .unwrap();
        let config = SecurityConfig {
            enabled: true,
            jwt: scafra::JwtConfig {
                enabled: true,
                secret: Some("configured-signing-secret".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let output = captured_logs();
        async {
            let response = send(
                guarded(config),
                "/v2/protected",
                Some(&format!("Bearer {token}")),
            )
            .await;
            assert_eq!(response.headers()["www-authenticate"], "Bearer");
            assert_error(
                response,
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication required",
            )
            .await;
        }
        .await;
        let logs = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
        for secret in [
            SECRET,
            token.as_str(),
            "configured-signing-secret",
            "wrong-signing-secret",
        ] {
            assert!(!logs.contains(secret));
        }
    }
}
