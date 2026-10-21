//! Exercise actual macro-produced handlers through Axum composition, not a URI
//! policy table reconstructed by this test.
#![cfg(feature = "jwt")]

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use scafra::prelude::*;
use scafra::web::{ControllerRoutes, RouterOptions};
use serde::Serialize;
use tower::ServiceExt;

#[controller("/protected", roles("admin"))]
#[derive(Default)]
struct Protected;
#[routes]
impl Protected {
    #[get("/")]
    async fn root(&self) -> &'static str {
        "admin data"
    }
    #[get("/open")]
    #[public]
    async fn open(&self) -> &'static str {
        "public"
    }
    #[get("/items/{id}")]
    async fn item(&self, _id: Path<String>) -> &'static str {
        "item"
    }
    #[get("/items/me")]
    #[public]
    async fn me(&self) -> &'static str {
        "me"
    }
    #[get("/files/{*path}")]
    async fn file(&self, _path: Path<String>) -> &'static str {
        "file"
    }
    #[get("/scoped")]
    #[scopes("reports.read")]
    async fn scoped(&self) -> &'static str {
        "scoped"
    }
    #[get("/slash/")]
    async fn slash(&self) -> &'static str {
        "slash"
    }
}

#[controller("/reports", scopes("reports.read"))]
#[derive(Default)]
struct Reports;
#[routes]
impl Reports {
    #[get("/")]
    async fn root(&self) -> &'static str {
        "reports"
    }
}

fn options() -> RouterOptions {
    RouterOptions {
        security: scafra::SecurityConfig {
            enabled: true,
            bearer_token: Some("shared-token".into()),
            basic: scafra::BasicAuthConfig {
                username: Some("reader".into()),
                password: Some("password".into()),
            },
            jwt: scafra::JwtConfig {
                enabled: true,
                secret: Some("nested-policy-secret".into()),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

#[derive(Serialize)]
struct Claims<'a> {
    exp: usize,
    roles: &'a [&'a str],
    scope: &'a str,
}
fn token(roles: &[&str], scope: &str) -> String {
    format!(
        "Bearer {}",
        encode(
            &Header::default(),
            &Claims {
                exp: 4_000_000_000,
                roles,
                scope
            },
            &EncodingKey::from_secret(b"nested-policy-secret")
        )
        .unwrap()
    )
}
async fn status(router: &Router, method: Method, path: &str, auth: Option<&str>) -> StatusCode {
    let mut request = Request::builder().uri(path).method(method);
    if let Some(auth) = auth {
        request = request.header("authorization", auth);
    }
    router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

fn nested(finalize_before: bool, repeat: bool) -> Router {
    let base = Protected::register_routes(Router::new());
    let base = if finalize_before {
        scafra::web::finish_managed_router(base, Protected::route_metadata(), &options())
            .unwrap()
            .into_router()
    } else {
        base
    };
    let base = Router::new().nest("/v2", base);
    let base = if repeat {
        Router::new().nest("/api", base)
    } else {
        base
    };
    scafra::web::finish_managed_router(base, Protected::route_metadata(), &options())
        .unwrap()
        .into_router()
}

#[tokio::test]
async fn nested_controller_retains_roles_scopes_public_overrides_and_exact_route_identity() {
    for finalize_before in [false, true] {
        for repeat in [false, true] {
            let router = nested(finalize_before, repeat);
            let prefix = if repeat {
                "/api/v2/protected"
            } else {
                "/v2/protected"
            };
            for suffix in ["", "/items/42", "/files/a/b", "/slash/"] {
                let path = format!("{prefix}{suffix}");
                assert_eq!(
                    status(&router, Method::GET, &path, None).await,
                    StatusCode::UNAUTHORIZED,
                    "{path}"
                );
                for auth in ["Basic cmVhZGVyOnBhc3N3b3Jk"] {
                    assert_eq!(
                        status(&router, Method::GET, &path, Some(auth)).await,
                        StatusCode::FORBIDDEN,
                        "{path}: {auth}"
                    );
                }
                assert_eq!(
                    status(&router, Method::GET, &path, Some(&token(&[], ""))).await,
                    StatusCode::FORBIDDEN,
                    "{path}"
                );
                assert_eq!(
                    status(&router, Method::GET, &path, Some(&token(&["admin"], ""))).await,
                    StatusCode::OK,
                    "{path}"
                );
                assert_eq!(
                    status(&router, Method::HEAD, &path, Some(&token(&[], ""))).await,
                    StatusCode::FORBIDDEN,
                    "HEAD {path}"
                );
            }
            for suffix in ["/open", "/items/me"] {
                assert_eq!(
                    status(&router, Method::GET, &format!("{prefix}{suffix}"), None).await,
                    StatusCode::OK
                );
            }
            let path = format!("{prefix}/scoped");
            assert_eq!(
                status(&router, Method::GET, &path, Some(&token(&["admin"], ""))).await,
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                status(
                    &router,
                    Method::GET,
                    &path,
                    Some(&token(&[], "reports.read"))
                )
                .await,
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                status(
                    &router,
                    Method::GET,
                    &path,
                    Some(&token(&["admin"], "reports.read"))
                )
                .await,
                StatusCode::OK
            );
            assert_eq!(
                status(
                    &router,
                    Method::GET,
                    prefix,
                    Some("Bearer invalid.jwt.signature")
                )
                .await,
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                status(
                    &router,
                    Method::GET,
                    &format!("{prefix}/slash"),
                    Some(&token(&["admin"], ""))
                )
                .await,
                StatusCode::NOT_FOUND
            );
        }
    }
}

#[tokio::test]
async fn differently_nested_controllers_do_not_share_policy_and_handwritten_routes_remain_distinct()
{
    let routes: Vec<_> = Protected::route_metadata()
        .iter()
        .chain(Reports::route_metadata())
        .copied()
        .collect();
    let router = Router::new()
        .nest("/one", Protected::register_routes(Router::new()))
        .nest("/two", Reports::register_routes(Router::new()))
        .merge(scafra::web::guard_axum_router(Router::new().route(
            "/handwritten",
            axum::routing::get(|| async { "ordinary" }),
        )));
    let router = scafra::web::finish_managed_router(router, &routes, &options())
        .unwrap()
        .into_router();
    assert_eq!(
        status(
            &router,
            Method::GET,
            "/one/protected",
            Some(&token(&["admin"], ""))
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        status(
            &router,
            Method::GET,
            "/two/reports",
            Some(&token(&["admin"], ""))
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(
            &router,
            Method::GET,
            "/two/reports",
            Some(&token(&[], "reports.read"))
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        status(&router, Method::GET, "/handwritten", Some(&token(&[], ""))).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn disabled_security_cannot_finalize_a_nested_protected_controller() {
    let router = Router::new().nest("/v2", Protected::register_routes(Router::new()));
    let mut options = options();
    options.security.enabled = false;
    assert!(
        scafra::web::finish_managed_router(router, Protected::route_metadata(), &options).is_err()
    );
}

#[tokio::test]
async fn nested_shared_bearer_authentication_does_not_grant_admin_role() {
    let mut options = options();
    options.security.jwt.enabled = false;
    let base = Protected::register_routes(Router::new());
    let router = scafra::web::finish_managed_router(
        Router::new().nest("/v2", base),
        Protected::route_metadata(),
        &options,
    )
    .unwrap()
    .into_router();
    assert_eq!(
        status(
            &router,
            Method::GET,
            "/v2/protected",
            Some("Bearer shared-token")
        )
        .await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn legacy_finalization_preserves_roles_after_nesting() {
    let mut options = options();
    options.security.jwt.enabled = false;
    let base = Protected::register_routes(Router::new());
    let router = scafra::web::finish_router_with_options(
        Router::new().nest("/v2", base),
        Protected::route_metadata(),
        &options,
    )
    .unwrap();
    assert_eq!(
        status(
            &router,
            Method::GET,
            "/v2/protected",
            Some("Bearer shared-token")
        )
        .await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn protected_controller_without_finalization_cannot_execute() {
    let router = Router::new().nest("/v2", Protected::register_routes(Router::new()));
    assert!(!status(
        &router,
        Method::GET,
        "/v2/protected",
        Some("Bearer shared-token")
    )
    .await
    .is_success());
}

#[controller("/implicit")]
#[derive(Default)]
struct Inherited;
#[routes]
impl Inherited {
    #[get("/public")]
    async fn public_name(&self) -> &'static str {
        "global policy applies"
    }
}

#[tokio::test]
async fn nested_global_permit_all_matches_original_uri_not_stripped_controller_path() {
    let mut options = options();
    options.security.permit_all = vec!["/implicit/public".into()];
    let router = scafra::web::finish_managed_router(
        Router::new().nest("/admin", Inherited::register_routes(Router::new())),
        Inherited::route_metadata(),
        &options,
    )
    .unwrap()
    .into_router();
    assert_eq!(
        status(&router, Method::GET, "/admin/implicit/public", None).await,
        StatusCode::UNAUTHORIZED
    );
    options.security.permit_all = vec!["/admin/implicit/public".into()];
    let router = scafra::web::finish_managed_router(
        Router::new().nest("/admin", Inherited::register_routes(Router::new())),
        Inherited::route_metadata(),
        &options,
    )
    .unwrap()
    .into_router();
    assert_eq!(
        status(&router, Method::GET, "/admin/implicit/public", None).await,
        StatusCode::OK
    );
}
