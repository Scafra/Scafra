//! Authorization declarations survive absence of the security implementation.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use scafra_web::{
    finish_router_with_options, AuthorizationMode, AuthorizationPolicy,
    ControllerAuthorizationRegistration, RouteAuthorizationMetadata, RouteMetadata, RouterOptions,
};
use tower::ServiceExt;

const fn metadata(path: &'static str, policy: AuthorizationPolicy) -> RouteAuthorizationMetadata {
    RouteAuthorizationMetadata {
        controller: "optional-security",
        method: "GET",
        prefix: "",
        path,
        controller_policy: AuthorizationPolicy::inherit(),
        route_policy: policy,
    }
}
inventory::submit! {
    ControllerAuthorizationRegistration { controller: "optional-security", routes: &[
        metadata("/protected", AuthorizationPolicy::protected()),
        RouteAuthorizationMetadata { controller_policy: AuthorizationPolicy::protected(), ..metadata("/inherited", AuthorizationPolicy::inherit()) },
        RouteAuthorizationMetadata { controller_policy: AuthorizationPolicy::protected(), ..metadata("/public", AuthorizationPolicy::public()) },
        metadata("/roles", AuthorizationPolicy { mode: AuthorizationMode::Inherit, role_groups: &[&["admin"]], required_scopes: &[] }),
        metadata("/malformed", AuthorizationPolicy { mode: AuthorizationMode::Public, role_groups: &[&["admin"]], required_scopes: &[] }),
        metadata("/scopes", AuthorizationPolicy { mode: AuthorizationMode::Inherit, role_groups: &[], required_scopes: &["read"] }),
        metadata("/conflict", AuthorizationPolicy::protected()),
        metadata("/conflict", AuthorizationPolicy::public()),
    ] }
}

#[test]
fn conflicting_authorization_records_cannot_override_protection() {
    let error = finish_router_with_options(
        Router::new(),
        &[route("/conflict")],
        &RouterOptions::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("duplicate authorization"));
}
fn route(path: &'static str) -> RouteMetadata {
    RouteMetadata {
        controller: "optional-security",
        method: "GET",
        prefix: "",
        path,
    }
}
#[test]
fn protection_is_rejected_when_security_unavailable_or_disabled() {
    for path in [
        "/protected",
        "/inherited",
        "/roles",
        "/scopes",
        "/malformed",
    ] {
        let result =
            finish_router_with_options(Router::new(), &[route(path)], &RouterOptions::default());
        assert!(result.is_err(), "{path} must fail closed");
        assert!(result.unwrap_err().to_string().contains("security"));
    }
}
#[tokio::test]
async fn plain_http_works_without_optional_integrations() {
    let router = finish_router_with_options(
        Router::new().route("/plain", get(|| async { "ok" })),
        &[route("/plain")],
        &RouterOptions::default(),
    )
    .unwrap();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/plain")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
#[test]
fn application_route_collisions_are_rejected_without_actuator() {
    let routes = [route("/collision"), route("/collision")];
    assert!(scafra_web::validate_routes_with_options(&routes, &RouterOptions::default()).is_err());
}

#[cfg(feature = "security")]
#[tokio::test]
async fn basic_authentication_requires_valid_credentials_without_jwt() {
    let options = RouterOptions {
        security: scafra_web::SecurityConfig {
            enabled: true,
            basic: scafra_web::BasicAuthConfig {
                username: Some("user".into()),
                password: Some("pass".into()),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let router = finish_router_with_options(
        Router::new().route("/protected", get(|| async { "authenticated" })),
        &[route("/protected")],
        &options,
    )
    .unwrap();
    for (authorization, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("Basic dXNlcjp3cm9uZw=="), StatusCode::UNAUTHORIZED),
        (Some("Basic dXNlcjpwYXNz"), StatusCode::OK),
    ] {
        let mut builder = Request::builder().uri("/protected");
        if let Some(authorization) = authorization {
            builder = builder.header("authorization", authorization);
        }
        let response = router
            .clone()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[cfg(all(feature = "security", not(feature = "jwt")))]
#[test]
fn explicitly_configured_jwt_is_rejected_even_when_security_disabled() {
    let mut options = RouterOptions::default();
    options.security.jwt.enabled = true;
    let error = finish_router_with_options(Router::new(), &[], &options).unwrap_err();
    assert!(error.to_string().contains("jwt"));
}

#[test]
fn explicit_public_override_remains_supported() {
    assert!(finish_router_with_options(
        Router::new(),
        &[route("/public")],
        &RouterOptions::default()
    )
    .is_ok());
}
