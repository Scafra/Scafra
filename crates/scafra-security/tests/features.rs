//! These tests also run without JWT, so feature absence itself is exercised.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
#[cfg(not(feature = "jwks"))]
use scafra_security::{layer, JwtConfig};
use scafra_security::{
    layer_with_route_policies, AuthorizationPolicy, RouteAuthorizationMetadata, SecurityConfig,
};
use tower::ServiceExt;

#[cfg(not(feature = "jwt"))]
#[test]
fn absent_jwt_rejects_explicit_intent_without_disclosing_secrets() {
    for jwt in [
        JwtConfig {
            enabled: true,
            ..JwtConfig::default()
        },
        JwtConfig {
            secret: Some("secret-sentinel".into()),
            ..JwtConfig::default()
        },
        JwtConfig {
            audiences: vec!["audience".into()],
            ..JwtConfig::default()
        },
    ] {
        let config = SecurityConfig {
            jwt,
            ..SecurityConfig::default()
        };
        let error = config.validate().unwrap_err();
        assert_eq!(error, "JWT configuration requires the `jwt` feature");
        assert!(!format!("{error} {config:?}").contains("secret-sentinel"));
    }
}

#[cfg(all(feature = "jwt", not(feature = "jwks")))]
#[test]
fn local_jwt_rejects_remote_intent_but_supports_local_issuer_validation() {
    for jwt in [
        JwtConfig {
            issuer_uri: Some("https://issuer.example".into()),
            ..JwtConfig::default()
        },
        JwtConfig {
            jwk_set_uri: Some("https://keys.example".into()),
            ..JwtConfig::default()
        },
    ] {
        let config = SecurityConfig {
            jwt,
            ..SecurityConfig::default()
        };
        assert_eq!(
            config.validate(),
            Err("remote JWT configuration requires the `jwks` feature")
        );
    }
    assert!(SecurityConfig {
        jwt: JwtConfig {
            enabled: true,
            secret: Some("secret".into()),
            issuer_uri: Some("https://issuer.example".into()),
            ..JwtConfig::default()
        },
        ..SecurityConfig::default()
    }
    .validate()
    .is_ok());
}

#[cfg(not(feature = "jwks"))]
#[tokio::test]
async fn unavailable_verifier_never_falls_back_to_static_bearer_or_basic() {
    let config = SecurityConfig {
        enabled: true,
        bearer_token: Some("fallback".into()),
        basic: scafra_security::BasicAuthConfig {
            username: Some("u".into()),
            password: Some("p".into()),
        },
        jwt: JwtConfig {
            enabled: true,
            jwk_set_uri: Some("https://keys.example".into()),
            ..JwtConfig::default()
        },
        ..SecurityConfig::default()
    };
    let router = layer(
        Router::new().route("/", get(|| async { "private" })),
        config,
    );
    for credential in ["Bearer fallback", "Basic dTpw"] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("authorization", credential)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[cfg(not(feature = "jwt"))]
#[tokio::test]
async fn disabled_global_security_cannot_hide_unavailable_jwt_in_direct_layer() {
    let config = SecurityConfig {
        jwt: JwtConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let router = layer(
        Router::new().route("/", get(|| async { "private" })),
        config,
    );
    let response = router
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_metadata_requires_credentials_with_global_security_disabled() {
    let metadata = RouteAuthorizationMetadata {
        controller: "controller",
        method: "GET",
        prefix: "",
        path: "/",
        controller_policy: AuthorizationPolicy::inherit(),
        route_policy: AuthorizationPolicy::protected(),
    };
    let router = layer_with_route_policies(
        Router::new().route("/", get(|| async { "private" })),
        SecurityConfig::default(),
        vec![metadata],
    );
    let response = router
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn inherited_role_and_scope_requirements_cannot_be_bypassed_by_permit_all() {
    for requirement in [
        AuthorizationPolicy {
            role_groups: &[&["admin"]],
            ..AuthorizationPolicy::inherit()
        },
        AuthorizationPolicy {
            required_scopes: &["read"],
            ..AuthorizationPolicy::inherit()
        },
    ] {
        let metadata = RouteAuthorizationMetadata {
            controller: "manual",
            method: "GET",
            prefix: "",
            path: "/",
            controller_policy: AuthorizationPolicy::public(),
            route_policy: requirement,
        };
        let config = SecurityConfig {
            enabled: true,
            bearer_token: Some("token".into()),
            permit_all: vec!["/".into()],
            ..SecurityConfig::default()
        };
        let router = layer_with_route_policies(
            Router::new().route("/", get(|| async { "private" })),
            config,
            vec![metadata],
        );
        for (token, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some("Bearer token"), StatusCode::FORBIDDEN),
        ] {
            let mut request = Request::builder().uri("/");
            if let Some(token) = token {
                request = request.header("authorization", token);
            }
            assert_eq!(
                router
                    .clone()
                    .oneshot(request.body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status(),
                expected
            );
        }
    }
}

#[tokio::test]
async fn explicit_head_protection_does_not_inherit_public_get() {
    let router = Router::new().route("/", get(|| async { "public" }).head(|| async { "private" }));
    let record = |method, policy| RouteAuthorizationMetadata {
        controller: "manual",
        method,
        prefix: "",
        path: "/",
        controller_policy: AuthorizationPolicy::inherit(),
        route_policy: policy,
    };
    let router = layer_with_route_policies(
        router,
        SecurityConfig {
            enabled: true,
            bearer_token: Some("token".into()),
            ..Default::default()
        },
        vec![
            record("GET", AuthorizationPolicy::public()),
            record("HEAD", AuthorizationPolicy::protected()),
        ],
    );
    for (method, expected) in [("GET", StatusCode::OK), ("HEAD", StatusCode::UNAUTHORIZED)] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn duplicate_policy_records_deny_in_both_orders() {
    for policies in [
        [
            AuthorizationPolicy::public(),
            AuthorizationPolicy::protected(),
        ],
        [
            AuthorizationPolicy::protected(),
            AuthorizationPolicy::public(),
        ],
    ] {
        let records = policies
            .into_iter()
            .map(|policy| RouteAuthorizationMetadata {
                controller: "manual",
                method: "GET",
                prefix: "",
                path: "/",
                controller_policy: AuthorizationPolicy::inherit(),
                route_policy: policy,
            })
            .collect();
        let router = layer_with_route_policies(
            Router::new().route("/", get(|| async { "private" })),
            SecurityConfig {
                enabled: true,
                bearer_token: Some("token".into()),
                ..Default::default()
            },
            records,
        );
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", "Bearer token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn contradictory_public_requirements_do_not_disable_enforcement() {
    let metadata = RouteAuthorizationMetadata {
        controller: "manual",
        method: "GET",
        prefix: "",
        path: "/",
        controller_policy: AuthorizationPolicy::inherit(),
        route_policy: AuthorizationPolicy {
            role_groups: &[&["admin"]],
            ..AuthorizationPolicy::public()
        },
    };
    let router = layer_with_route_policies(
        Router::new().route("/", get(|| async { "private" })),
        SecurityConfig::default(),
        vec![metadata],
    );
    assert_eq!(
        router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn static_paths_take_precedence_over_public_parameter_policies() {
    use axum::routing::head;
    let record = |method, path, route_policy| RouteAuthorizationMetadata {
        controller: "manual",
        method,
        prefix: "",
        path,
        controller_policy: AuthorizationPolicy::inherit(),
        route_policy,
    };
    for (method, static_policy, parameter_is_head) in [
        ("GET", AuthorizationPolicy::inherit(), false),
        ("HEAD", AuthorizationPolicy::protected(), true),
    ] {
        let parameter = if parameter_is_head {
            head(|| async { "public parameter" })
        } else {
            get(|| async { "public parameter" })
        };
        let router = layer_with_route_policies(
            Router::new()
                .route("/users/me", get(|| async { "private static" }))
                .route("/users/{id}", parameter),
            SecurityConfig {
                enabled: true,
                bearer_token: Some("token".into()),
                ..Default::default()
            },
            vec![
                record("GET", "/users/me", static_policy),
                record(
                    if parameter_is_head { "HEAD" } else { "GET" },
                    "/users/{id}",
                    AuthorizationPolicy::public(),
                ),
            ],
        );
        for (uri, expected) in [
            ("/users/me", StatusCode::UNAUTHORIZED),
            ("/users/other", StatusCode::OK),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{method} {uri}");
        }
        assert_eq!(
            router
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/users/me")
                        .header("authorization", "Bearer token")
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
}
