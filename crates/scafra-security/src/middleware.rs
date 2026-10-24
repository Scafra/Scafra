//! Axum router middleware and HTTP authentication/authorization responses.

use crate::{
    authentication::{authenticate, Principal},
    config::SecurityConfig,
    jwks::JwksCache,
    policy::{
        matching_policy, resolve_policy, ResolvedPolicy, RouteAuthorizationMetadata,
        RoutePolicyMatcher,
    },
    AuthorizationMode, AuthorizationPolicy,
};
use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Router,
};
use scafra_foundation::http_error::HttpError;
use std::sync::Arc;

/// Shared authentication configuration and verifier cache for one application.
///
/// The outermost context is authoritative when routers are composed. Contexts
/// never expose credentials or authenticated principals through the public API.
#[derive(Clone)]
pub struct SecurityContext(Arc<SecurityState>);

struct SecurityState {
    invalid_configuration: bool,
    config: SecurityConfig,
    jwks: JwksCache,
}

#[derive(Clone)]
struct Authenticated {
    context: SecurityContext,
    principal: Arc<Principal>,
}

impl SecurityContext {
    fn new(config: SecurityConfig) -> Self {
        Self(Arc::new(SecurityState {
            invalid_configuration: config.validate().is_err(),
            config,
            jwks: JwksCache::new(),
        }))
    }

    fn install(&self, request: &mut Request<axum::body::Body>) -> Self {
        if let Some(context) = request.extensions().get::<Self>() {
            return context.clone();
        }
        request.extensions_mut().insert(self.clone());
        self.clone()
    }
}

/// Installs a shared security context without guessing policies from URI paths.
///
/// Managed routes enforce their own declarations with [`authorize_route`].
/// Handwritten routes must be guarded explicitly with [`guard_axum_router`] or
/// use the legacy application-wide [`layer`] API.
pub fn context_layer(router: Router, config: SecurityConfig) -> Router {
    let context = SecurityContext::new(config);
    router.layer(axum::middleware::from_fn(
        move |mut request: Request<axum::body::Body>, next: Next| {
            let context = context.install(&mut request);
            async move {
                if context.0.invalid_configuration {
                    return misconfigured(Some(&context.0));
                }
                next.run(request).await
            }
        },
    ))
}

/// Applies inherited application security to handwritten Axum routes.
///
/// Call this before merging managed controllers, whose local guards implement
/// explicit public overrides. The application must install a security context.
pub fn guard_axum_router(router: Router) -> Router {
    const INHERITED: RouteAuthorizationMetadata = RouteAuthorizationMetadata {
        controller: "",
        method: "",
        prefix: "",
        path: "",
        controller_policy: AuthorizationPolicy::inherit(),
        route_policy: AuthorizationPolicy::inherit(),
    };
    router.layer(axum::middleware::from_fn(
        |request: Request<axum::body::Body>, next| async move {
            if request.extensions().get::<SecurityContext>().is_none() {
                return misconfigured(None);
            }
            authorize_route(request, next, INHERITED).await
        },
    ))
}

/// Enforces the declaration attached to the actual selected handler.
///
/// This guard is independent of URI matching and survives Axum nesting. A
/// protected declaration without an installed security context fails closed.
pub async fn authorize_route(
    request: Request<axum::body::Body>,
    next: Next,
    metadata: RouteAuthorizationMetadata,
) -> Response {
    let context = request.extensions().get::<SecurityContext>().cloned();
    if !valid_metadata(&metadata) {
        return context.map_or_else(
            || misconfigured(None),
            |context| misconfigured(Some(&context.0)),
        );
    }
    let policy = resolve_policy(&metadata);
    let Some(context) = context else {
        return if policy.is_protected() {
            misconfigured(None)
        } else {
            next.run(request).await
        };
    };
    enforce(context, Some(policy), request, next).await
}

/// Applies application-wide security to an application router.
pub fn layer(router: Router, config: SecurityConfig) -> Router {
    layer_with_route_policies(router, config, Vec::new())
}

/// Applies application-wide and legacy URI-indexed authorization policies.
///
/// Generated route guards independently enforce their declarations. For
/// transformed managed routes use context-only finalization, avoiding an outer
/// URI policy preventing a route-local public override.
pub fn layer_with_route_policies(
    router: Router,
    config: SecurityConfig,
    policies: Vec<RouteAuthorizationMetadata>,
) -> Router {
    let context = SecurityContext::new(config);
    let mut seen = std::collections::HashSet::new();
    let invalid_policies = policies.iter().any(|policy| {
        !seen.insert((
            policy.method.to_ascii_uppercase(),
            crate::policy::join_paths(policy.prefix, policy.path),
        )) || !valid_metadata(policy)
    });
    let policies = Arc::new(RoutePolicyMatcher::new(policies));
    router.layer(axum::middleware::from_fn(
        move |mut request: Request<axum::body::Body>, next: Next| {
            let context = context.install(&mut request);
            let policies = Arc::clone(&policies);
            async move {
                if invalid_policies {
                    return misconfigured(Some(&context.0));
                }
                if !context.0.invalid_configuration
                    && !context.0.config.enabled
                    && !policies.has_protected_routes()
                {
                    return next.run(request).await;
                }
                let policy = if policies.is_empty() {
                    None
                } else {
                    matching_policy(&policies, request.method(), request.uri().path())
                };
                enforce(context, policy, request, next).await
            }
        },
    ))
}

fn valid_metadata(metadata: &RouteAuthorizationMetadata) -> bool {
    [metadata.controller_policy, metadata.route_policy]
        .iter()
        .all(|declaration| {
            !(declaration.mode == AuthorizationMode::Public
                && (!declaration.role_groups.is_empty() || !declaration.required_scopes.is_empty()))
                && declaration.role_groups.iter().all(|group| {
                    !group.is_empty() && group.iter().all(|role| !role.trim().is_empty())
                })
                && declaration
                    .required_scopes
                    .iter()
                    .all(|scope| !scope.trim().is_empty())
        })
}

async fn enforce(
    context: SecurityContext,
    policy: Option<ResolvedPolicy>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let state = &context.0;
    if state.invalid_configuration {
        return misconfigured(Some(state));
    }
    if policy.is_some_and(ResolvedPolicy::is_public) {
        return next.run(request).await;
    }
    let route_protected = policy.is_some_and(ResolvedPolicy::is_protected);
    // Axum rewrites the URI while descending through nested routers. Global
    // permit paths describe the externally requested path, not the local mount.
    let original_uri = request.extensions().get::<axum::extract::OriginalUri>();
    let path = original_uri.map_or_else(|| request.uri().path(), |uri| uri.0.path());
    let globally_protected = state.config.enabled && !state.config.is_permitted(path);
    if !route_protected && !globally_protected {
        return next.run(request).await;
    }
    // Disabled security cannot authenticate explicitly protected routes.
    if !state.config.enabled {
        return misconfigured(Some(state));
    }
    let cached = request
        .extensions()
        .get::<Authenticated>()
        .filter(|authenticated| Arc::ptr_eq(&authenticated.context.0, &context.0))
        .map(|authenticated| Arc::clone(&authenticated.principal));
    let principal = if let Some(principal) = cached {
        principal
    } else {
        let mut values = request
            .headers()
            .get_all(axum::http::header::AUTHORIZATION)
            .iter();
        let authorization = match (values.next(), values.next()) {
            (Some(_), Some(_)) => return denied(state, StatusCode::UNAUTHORIZED),
            (Some(value), None) => value.to_str().ok(),
            (None, _) => None,
        };
        let Some(principal) = authenticate(&state.config, &state.jwks, authorization).await else {
            return denied(state, StatusCode::UNAUTHORIZED);
        };
        let principal = Arc::new(principal);
        request.extensions_mut().insert(Authenticated {
            context: context.clone(),
            principal: Arc::clone(&principal),
        });
        principal
    };
    let policy = policy.unwrap_or_default();
    let mut scopes = policy
        .controller_scopes
        .iter()
        .chain(policy.route_scopes.iter())
        .copied()
        .chain(
            principal
                .is_jwt
                .then_some(state.config.jwt.required_scopes.iter().map(String::as_str))
                .into_iter()
                .flatten(),
        );
    let authorized = policy
        .controller_role_groups
        .iter()
        .chain(policy.route_role_groups.iter())
        .all(|group| group.iter().any(|role| principal.roles.contains(*role)))
        && scopes.all(|scope| principal.scopes.contains(scope));
    if !authorized {
        return denied(state, StatusCode::FORBIDDEN);
    }
    next.run(request).await
}

fn denied(state: &SecurityState, status: StatusCode) -> Response {
    if state.config.hide_unauthorized {
        StatusCode::NOT_FOUND.into_response()
    } else if status == StatusCode::UNAUTHORIZED {
        let mut response = error_response(HttpError::Unauthorized);
        // Advertise only configured schemes, including both when supported.
        if state.config.jwt.enabled || state.config.bearer_token.is_some() {
            response.headers_mut().append(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Bearer"),
            );
        }
        if state.config.basic.username.is_some() && state.config.basic.password.is_some() {
            response.headers_mut().append(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Basic realm=\"Scafra\""),
            );
        }
        if !response
            .headers()
            .contains_key(axum::http::header::WWW_AUTHENTICATE)
        {
            return misconfigured(Some(state));
        }
        response
    } else {
        error_response(HttpError::Forbidden)
    }
}

// Server-side security setup failures cannot be corrected by client credentials.
// Preserve concealment when its configuration is available, otherwise fail closed
// without issuing a 401 or advertising an authenticator that does not exist.
fn misconfigured(state: Option<&SecurityState>) -> Response {
    if state.is_some_and(|state| state.config.hide_unauthorized) {
        StatusCode::NOT_FOUND.into_response()
    } else {
        error_response(HttpError::Internal)
    }
}

fn error_response(error: HttpError) -> Response {
    let status = StatusCode::from_u16(error.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, axum::Json(error)).into_response()
}
