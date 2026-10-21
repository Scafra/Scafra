//! Route identity is carried by the handler guard, never reconstructed from URI.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use scafra_security::{
    authorize_route, context_layer, guard_axum_router, layer_with_route_policies,
    AuthorizationMode, AuthorizationPolicy, RouteAuthorizationMetadata, SecurityConfig,
};
use tower::ServiceExt;

const ADMIN: RouteAuthorizationMetadata = RouteAuthorizationMetadata {
    controller: "Protected",
    method: "GET",
    prefix: "/protected",
    path: "/",
    controller_policy: AuthorizationPolicy {
        mode: AuthorizationMode::Protected,
        role_groups: &[&["admin"]],
        required_scopes: &["data.read"],
    },
    route_policy: AuthorizationPolicy::inherit(),
};

fn guarded(metadata: RouteAuthorizationMetadata) -> Router {
    Router::new().route(
        "/protected/",
        get(|| async { "admin data" }).layer(middleware::from_fn(move |request, next| {
            authorize_route(request, next, metadata)
        })),
    )
}

fn bearer(token: &str) -> SecurityConfig {
    SecurityConfig {
        enabled: true,
        bearer_token: Some(token.to_owned()),
        ..SecurityConfig::default()
    }
}

async fn status(router: Router, path: &str, authorization: Option<&str>) -> StatusCode {
    let mut request = Request::builder().uri(path);
    if let Some(authorization) = authorization {
        request = request.header("authorization", authorization);
    }
    router
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn nested_handler_roles_do_not_disappear_after_authentication() {
    let nested = Router::new().nest("/v2", guarded(ADMIN));
    let app = context_layer(nested, bearer("valid"));
    assert_eq!(
        status(app.clone(), "/v2/protected/", None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(app.clone(), "/v2/protected/", Some("Bearer invalid")).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(app, "/v2/protected/", Some("Bearer valid")).await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn guard_without_context_or_with_disabled_context_fails_closed() {
    assert_eq!(
        status(guarded(ADMIN), "/protected/", Some("Bearer valid")).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(
            context_layer(guarded(ADMIN), SecurityConfig::default()),
            "/protected/",
            Some("Bearer valid")
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn nested_public_override_and_invalid_metadata() {
    let public = RouteAuthorizationMetadata {
        route_policy: AuthorizationPolicy::public(),
        ..ADMIN
    };
    let app = context_layer(Router::new().nest("/v2", guarded(public)), bearer("valid"));
    assert_eq!(status(app, "/v2/protected/", None).await, StatusCode::OK);
    let malformed = RouteAuthorizationMetadata {
        route_policy: AuthorizationPolicy {
            mode: AuthorizationMode::Public,
            role_groups: &[&["admin"]],
            required_scopes: &[],
        },
        ..ADMIN
    };
    assert_eq!(
        status(
            context_layer(guarded(malformed), bearer("valid")),
            "/protected/",
            Some("Bearer valid")
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn outermost_context_is_authoritative_for_legacy_and_local_enforcement() {
    let inner = context_layer(
        guarded(RouteAuthorizationMetadata {
            controller_policy: AuthorizationPolicy::protected(),
            ..ADMIN
        }),
        bearer("inner"),
    );
    let app = layer_with_route_policies(inner, bearer("outer"), vec![]);
    assert_eq!(
        status(app.clone(), "/protected/", Some("Bearer outer")).await,
        StatusCode::OK
    );
    assert_eq!(
        status(app, "/protected/", Some("Bearer inner")).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn handwritten_guard_inherits_global_security_without_claiming_controller_policy() {
    let handwritten =
        guard_axum_router(Router::new().route("/handwritten", get(|| async { "ok" })));
    let app = context_layer(handwritten, bearer("valid"));
    assert_eq!(
        status(app.clone(), "/handwritten", None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(app, "/handwritten", Some("Bearer valid")).await,
        StatusCode::OK
    );
}

#[cfg(feature = "jwt")]
#[tokio::test]
async fn repeated_nesting_preserves_jwt_roles_and_scopes() {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;
    #[derive(Serialize)]
    struct Claims<'a> {
        exp: usize,
        roles: &'a [&'a str],
        scope: &'a str,
    }
    let config = SecurityConfig {
        enabled: true,
        jwt: scafra_security::JwtConfig {
            enabled: true,
            secret: Some("route-local-test-secret".to_owned()),
            ..Default::default()
        },
        ..Default::default()
    };
    let app = context_layer(
        Router::new().nest("/outer", Router::new().nest("/v2", guarded(ADMIN))),
        config,
    );
    for (roles, scope, expected) in [
        (&[][..], "data.read", StatusCode::FORBIDDEN),
        (&["admin"][..], "", StatusCode::FORBIDDEN),
        (&["admin"][..], "data.read", StatusCode::OK),
    ] {
        let token = encode(
            &Header::default(),
            &Claims {
                exp: 4_000_000_000,
                roles,
                scope,
            },
            &EncodingKey::from_secret(b"route-local-test-secret"),
        )
        .unwrap();
        assert_eq!(
            status(
                app.clone(),
                "/outer/v2/protected/",
                Some(&format!("Bearer {token}"))
            )
            .await,
            expected
        );
    }
    assert_eq!(
        status(
            app,
            "/outer/v2/protected/",
            Some("Bearer malformed.jwt.token")
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[cfg(feature = "jwks")]
#[tokio::test]
async fn repeated_finalization_shares_one_effective_jwks_cache() {
    use axum::Json;
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let provider = Router::new().route("/keys", get(move || {
        let count = count.clone();
        async move {
            count.fetch_add(1, Ordering::SeqCst);
            Json(json!({"keys": [{
                "kid": "route-local", "kty": "RSA", "e": "AQAB",
                "n": "tEaY79VFcn9pvrOHgFR6RVjud03RgrxyuUOlJXJEF7-VuOsRk8350rygE1dQ0eQo-O6J3JxR5DzZh9Bd6zCQ2_7Jp6BWo26b6x0ZtnLKGQkezc5rbshS9y2r7SnyCsrAS9G_IGQpy2pnRvQq0_BlSnIo3qH8MarecboZl-sJ4PLDDNioC3cD6HkF5rCvyh_-YZGMO8t8DfPJX0erTTnE8ss69-WQq6BBA6HzVx2HcYpaP_hg_WN30ozenvTYGpCCdAAtzpuefpZlQn-jI5ZIvnAsBG7627rTH91GhHkbrVAZPPkQq5-G1zl6qfoD7tVONGRLCucx8qZvKzkHesiG6w"
            }]}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let provider_task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let config = SecurityConfig {
        enabled: true,
        jwt: scafra_security::JwtConfig {
            enabled: true,
            issuer_uri: Some(issuer.clone()),
            jwk_set_uri: Some(format!("{issuer}/keys")),
            ..Default::default()
        },
        ..Default::default()
    };
    let inner = layer_with_route_policies(guarded(ADMIN), config.clone(), vec![ADMIN]);
    let app = layer_with_route_policies(context_layer(inner, config.clone()), config, vec![ADMIN]);
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("route-local".to_owned());
    let token = encode(&header, &json!({"exp": 4_000_000_000usize, "iss": issuer, "roles": ["admin"], "scope": "data.read"}), &EncodingKey::from_rsa_pem(include_bytes!("fixtures/rsa-private.pem")).unwrap()).unwrap();
    for _ in 0..2 {
        assert_eq!(
            status(app.clone(), "/protected/", Some(&format!("Bearer {token}"))).await,
            StatusCode::OK
        );
    }
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "inner finalizers must reuse the outer verifier and cached principal"
    );
    provider_task.abort();
    let _ = provider_task.await;
}

#[tokio::test]
async fn inherited_global_permits_use_the_original_external_uri() {
    let inherited = RouteAuthorizationMetadata {
        controller_policy: AuthorizationPolicy::inherit(),
        ..ADMIN
    };
    let mut config = bearer("valid");
    config.permit_all = vec!["/protected/".to_owned()];
    let nested = Router::new().nest("/admin", guarded(inherited));
    assert_eq!(
        status(
            context_layer(nested.clone(), config.clone()),
            "/admin/protected/",
            None
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    config.permit_all = vec!["/admin/protected/".to_owned()];
    assert_eq!(
        status(context_layer(nested, config), "/admin/protected/", None).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn explicit_handwritten_guard_requires_an_installed_context() {
    let router = guard_axum_router(Router::new().route("/handwritten", get(|| async { "ok" })));
    assert_eq!(
        status(router, "/handwritten", None).await,
        StatusCode::UNAUTHORIZED
    );
}
