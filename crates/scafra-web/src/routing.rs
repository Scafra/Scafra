use std::collections::HashMap;
use std::time::Duration;

use axum::{extract::DefaultBodyLimit, http::StatusCode, Router};
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

use crate::{errors::WebError, registration::ControllerRegistration};
#[cfg(feature = "actuator")]
use scafra_actuator::{ActuatorConfig, HealthRegistry};

/// Joins a controller prefix and method path while preserving Axum route
/// syntax, including path parameters such as {id}.
pub fn join_paths(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".to_owned(),
        (true, false) => format!("/{path}"),
        (false, true) => prefix.to_owned(),
        (false, false) => format!("{prefix}/{path}"),
    }
}

/// Selected integrations and common HTTP middleware settings.
#[derive(Debug, Clone, Default)]
pub struct RouterOptions {
    #[cfg(feature = "actuator")]
    pub actuator: ActuatorConfig,
    #[cfg(feature = "security")]
    pub security: scafra_security::SecurityConfig,
    pub request_timeout: Option<Duration>,
}

/// Builds statically registered controllers using default integration options.
pub fn build_router() -> Result<Router, WebError> {
    build_router_with_options(&RouterOptions::default())
}

/// Creates only the selected operational endpoints.
pub fn operational_router(options: &RouterOptions) -> Result<Router, WebError> {
    #[cfg(feature = "actuator")]
    {
        Ok(scafra_actuator::try_router(&options.actuator)?)
    }
    #[cfg(not(feature = "actuator"))]
    {
        let _ = options;
        Ok(Router::new())
    }
}

/// Builds registered controllers, validates declarations, then applies middleware.
pub fn build_router_with_options(options: &RouterOptions) -> Result<Router, WebError> {
    let routes = default_controller_metadata(&[]);
    validate_routes_with_options(&routes, options)?;
    let router = register_default_controllers(operational_router(options)?, &[]);
    finish_router_with_options(router, &routes, options)
}

/// Validates route protection even when the security implementation is absent.
///
/// `routes` must describe every framework controller installed in `router`,
/// using unchanged declared prefixes/paths. This API cannot infer metadata for
/// arbitrary Axum routes or transform it after nesting/path rewrites.
pub fn finish_router_with_options(
    router: Router,
    routes: &[crate::registration::RouteMetadata],
    options: &RouterOptions,
) -> Result<Router, WebError> {
    validate_routes_with_options(routes, options)?;
    apply_middleware(router, routes, options)
}

// An opaque supplied Axum router already resolves installed route collisions.
// Linked registrations can include unused adapters, so only validate/enforce
// their authorization here rather than treating them as installed route pairs.
pub(crate) fn finish_supplied_router(
    router: Router,
    options: &RouterOptions,
) -> Result<Router, WebError> {
    apply_middleware(router, &default_controller_metadata(&[]), options)
}

fn apply_middleware(
    router: Router,
    routes: &[crate::registration::RouteMetadata],
    options: &RouterOptions,
) -> Result<Router, WebError> {
    let policies = authorization_metadata_for(routes);
    validate_authorization_metadata(&policies)?;
    #[cfg(feature = "security")]
    validate_security_config(&options.security, &policies)?;
    #[cfg(not(feature = "security"))]
    if policies.iter().any(policy_requires_security) {
        return Err(security_configuration_error(
            "protected routes require the security Cargo feature",
        ));
    }
    let router = router.layer(DefaultBodyLimit::max(1024 * 1024));
    #[cfg(feature = "security")]
    let router =
        scafra_security::layer_with_route_policies(router, options.security.clone(), policies);
    let router = with_request_timeout(router, options.request_timeout);
    Ok(router.layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
        tracing::debug_span!("request", method = %request.method(), version = ?request.version())
    })))
}

fn policy_requires_security(policy: &scafra_foundation::RouteAuthorizationMetadata) -> bool {
    use scafra_foundation::AuthorizationMode;
    if policy.route_policy.mode == AuthorizationMode::Public {
        return false;
    }
    if policy.route_policy.mode == AuthorizationMode::Protected {
        return true;
    }
    policy.controller_policy.mode == AuthorizationMode::Protected
        || !policy.controller_policy.role_groups.is_empty()
        || !policy.controller_policy.required_scopes.is_empty()
        || !policy.route_policy.role_groups.is_empty()
        || !policy.route_policy.required_scopes.is_empty()
}

fn validate_authorization_metadata(
    policies: &[scafra_foundation::RouteAuthorizationMetadata],
) -> Result<(), WebError> {
    use scafra_foundation::AuthorizationMode;
    let mut seen = std::collections::HashSet::new();
    for metadata in policies {
        if !seen.insert((
            metadata.method.to_ascii_uppercase(),
            join_paths(metadata.prefix, metadata.path),
        )) {
            return Err(security_configuration_error(
                "duplicate authorization metadata for one method and path",
            ));
        }
        for policy in [metadata.controller_policy, metadata.route_policy] {
            if policy.mode == AuthorizationMode::Public
                && (!policy.role_groups.is_empty() || !policy.required_scopes.is_empty())
            {
                return Err(security_configuration_error(
                    "public policy cannot require roles or scopes",
                ));
            }
            if policy
                .role_groups
                .iter()
                .any(|group| group.is_empty() || group.iter().any(|role| role.trim().is_empty()))
                || policy
                    .required_scopes
                    .iter()
                    .any(|scope| scope.trim().is_empty())
            {
                return Err(security_configuration_error(
                    "authorization roles and scopes must be nonempty",
                ));
            }
        }
    }
    Ok(())
}

/// Builds the application router with configurable operational endpoints and
/// legacy process-wide health registrations.
///
/// Use [`build_router_with_actuator_and_health_registry`] to pass checks owned
/// by one application graph.
#[cfg(feature = "actuator")]
pub fn build_router_with_actuator(actuator: &ActuatorConfig) -> Result<Router, WebError> {
    build_router_with_options(&RouterOptions {
        actuator: actuator.clone(),
        ..Default::default()
    })
}

/// Builds the application router with legacy process-wide health registrations
/// and the supplied application security configuration.
///
/// Use [`build_router_with_actuator_and_health_registry_and_security`] to pass
/// checks owned by one application graph.
#[cfg(all(feature = "actuator", feature = "security"))]
pub fn build_router_with_actuator_and_security(
    actuator: &ActuatorConfig,
    security: &scafra_security::SecurityConfig,
) -> Result<Router, WebError> {
    build_router_with_timeout(actuator, security, None)
}

/// Builds the default application router with checks owned by one graph.
#[cfg(feature = "actuator")]
pub fn build_router_with_actuator_and_health_registry(
    actuator: &ActuatorConfig,
    registry: HealthRegistry,
) -> Result<Router, WebError> {
    let options = RouterOptions {
        actuator: actuator.clone(),
        ..Default::default()
    };
    let routes = default_controller_metadata(&[]);
    validate_routes_with_options(&routes, &options)?;
    let router = scafra_actuator::router_with_health_registry(actuator, registry)?;
    finish_router_with_options(register_default_controllers(router, &[]), &routes, &options)
}

/// Builds the default application router with graph-scoped health checks and
/// the supplied application security configuration.
#[cfg(all(feature = "actuator", feature = "security"))]
pub fn build_router_with_actuator_and_health_registry_and_security(
    actuator: &ActuatorConfig,
    registry: HealthRegistry,
    security: &scafra_security::SecurityConfig,
) -> Result<Router, WebError> {
    let router = scafra_actuator::router_with_health_registry(actuator, registry)?;
    build_router_with_actuator_router(router, actuator, security, None)
}

#[cfg(all(feature = "actuator", feature = "security"))]
pub(crate) fn build_router_with_timeout(
    actuator: &ActuatorConfig,
    security: &scafra_security::SecurityConfig,
    request_timeout: Option<Duration>,
) -> Result<Router, WebError> {
    let router = scafra_actuator::try_router(actuator)?;
    build_router_with_actuator_router(router, actuator, security, request_timeout)
}

#[cfg(all(feature = "actuator", feature = "security"))]
fn build_router_with_actuator_router(
    router: Router,
    actuator: &ActuatorConfig,
    security: &scafra_security::SecurityConfig,
    request_timeout: Option<Duration>,
) -> Result<Router, WebError> {
    let routes = default_controller_metadata(&[]);
    validate_routes(&routes, actuator)?;
    let router = register_default_controllers(router, &[]);
    finish_router(router, &routes, actuator, security, request_timeout)
}

/// Compatibility wrapper for integrated applications.
#[cfg(all(feature = "actuator", feature = "security"))]
pub fn finish_router(
    router: Router,
    routes: &[crate::registration::RouteMetadata],
    actuator: &ActuatorConfig,
    security: &scafra_security::SecurityConfig,
    request_timeout: Option<Duration>,
) -> Result<Router, WebError> {
    finish_router_with_options(
        router,
        routes,
        &RouterOptions {
            actuator: actuator.clone(),
            security: security.clone(),
            request_timeout,
        },
    )
}

fn authorization_metadata_for(
    routes: &[crate::registration::RouteMetadata],
) -> Vec<scafra_foundation::RouteAuthorizationMetadata> {
    inventory::iter::<scafra_foundation::ControllerAuthorizationRegistration>()
        .flat_map(|registration| registration.routes.iter().copied())
        .filter(|policy| {
            routes.iter().any(|route| {
                route.controller == policy.controller
                    && route.method == policy.method
                    && route.prefix == policy.prefix
                    && route.path == policy.path
            })
        })
        .collect()
}

#[cfg(feature = "security")]
fn validate_security_config(
    security: &scafra_security::SecurityConfig,
    policies: &[scafra_foundation::RouteAuthorizationMetadata],
) -> Result<(), WebError> {
    security.validate().map_err(security_configuration_error)?;
    let has_protected_route = policies.iter().any(policy_requires_security);
    if has_protected_route && !security.enabled {
        return Err(security_configuration_error(
            "security must be enabled when a controller or route requires authentication",
        ));
    }
    Ok(())
}

fn security_configuration_error(reason: &'static str) -> WebError {
    WebError::Server(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("invalid security configuration: {reason}"),
    ))
}

/// Compatibility validation with explicit actuator configuration.
#[cfg(feature = "actuator")]
pub fn validate_routes(
    routes: &[crate::registration::RouteMetadata],
    actuator: &ActuatorConfig,
) -> Result<(), WebError> {
    validate_routes_with_options(
        routes,
        &RouterOptions {
            actuator: actuator.clone(),
            ..Default::default()
        },
    )
}

/// Checks collisions against endpoints owned by the selected integrations.
pub fn validate_routes_with_options(
    routes: &[crate::registration::RouteMetadata],
    options: &RouterOptions,
) -> Result<(), WebError> {
    let mut seen = HashMap::<(String, String), &'static str>::new();
    #[cfg(feature = "actuator")]
    for (method, path) in scafra_actuator::reserved_routes(&options.actuator) {
        seen.insert(
            ((*method).to_owned(), (*path).to_owned()),
            "scafra-actuator",
        );
    }
    #[cfg(not(feature = "actuator"))]
    let _ = options;
    for route in routes {
        let key = (
            route.method.to_owned(),
            join_paths(route.prefix, route.path),
        );
        if let Some(first) = seen.insert(key.clone(), route.controller) {
            return Err(WebError::DuplicateRoute {
                method: key.0,
                path: key.1,
                first,
                second: route.controller,
            });
        }
    }
    Ok(())
}

/// Registers statically linked route adapters that were not included in typed
/// graph composition. Handwritten adapters can retain legacy behavior here;
/// generated controllers are registered from constructed graph instances.
pub fn register_default_controllers(mut router: Router, composed_controllers: &[&str]) -> Router {
    for registration in inventory::iter::<ControllerRegistration>() {
        if composed_controllers.contains(&registration.controller) {
            continue;
        }
        router = (registration.register)(router);
    }
    router
}

/// Returns route metadata for linked controllers, excluding controllers that
/// typed composition will register itself.
pub fn default_controller_metadata(
    composed_controllers: &[&str],
) -> Vec<crate::registration::RouteMetadata> {
    inventory::iter::<ControllerRegistration>()
        .filter(|registration| !composed_controllers.contains(&registration.controller))
        .flat_map(|registration| registration.routes.iter().copied())
        .collect()
}

pub(crate) fn with_request_timeout(router: Router, request_timeout: Option<Duration>) -> Router {
    match request_timeout {
        Some(timeout) => router.layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            timeout,
        )),
        None => router,
    }
}
