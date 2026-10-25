//! Axum/Tokio integration for Scafra.
//!
//! The default feature selection provides plain HTTP, JSON, tracing, request
//! limits, and graceful shutdown. `security` adds Basic/shared-bearer security,
//! `jwt` adds local JWT validation, `jwks` adds remote key retrieval, and
//! `actuator` adds operational endpoints. `standard` selects all integrations.
//!
//! Use [`RouterOptions`] to compose only the integrations compiled into the
//! application. Authorization declarations always remain available: router
//! construction rejects protected routes when security is absent or disabled.
//! Generated route-local guards preserve protection through Axum composition.
//! [`finish_managed_router`] explicitly composes them with a shared context;
//! handwritten routes need [`guard_axum_router`] before merging. Legacy finish
//! and raw serving APIs retain application-wide enforcement for arbitrary routes.

mod connection;
mod errors;
mod json;
mod limits;
mod registration;
mod routing;
mod server;

pub use axum;
#[cfg(feature = "actuator")]
pub use scafra_actuator;
pub use scafra_core;
pub use tower;

/// Internal re-exports used by generated code. They are public only so a
/// downstream application does not need to depend on Scafra's implementation
/// crates directly.
#[doc(hidden)]
pub mod __private {
    pub use crate::routing::authorize_route;
    pub use crate::routing::register_default_controllers_with_options;
    pub use inventory;
}

pub use connection::ConnectionOptions;
pub use errors::{AppError, AuthenticationChallenge, ServerError, WebError};
pub use json::JsonBody;
pub use limits::with_request_body_limit;
pub use registration::{ControllerPrefix, ControllerRegistration, ControllerRoutes, RouteMetadata};
pub use routing::{
    build_router, build_router_with_options, default_controller_metadata, finish_managed_router,
    finish_router_with_options, guard_axum_router, join_paths, operational_router,
    register_default_controllers, validate_routes_with_options, with_request_timeout,
    FinalizedRouter, RouterOptions,
};
#[cfg(feature = "actuator")]
pub use routing::{
    build_router_with_actuator, build_router_with_actuator_and_health_registry, validate_routes,
};
#[cfg(all(feature = "actuator", feature = "security"))]
pub use routing::{
    build_router_with_actuator_and_health_registry_and_security,
    build_router_with_actuator_and_security, finish_router,
};
#[cfg(feature = "actuator")]
pub use scafra_actuator::register_health_check;
#[cfg(feature = "actuator")]
pub use scafra_actuator::{
    ActuatorConfig, ActuatorSecurity, EndpointSelection, HealthCheckFuture,
    HealthCheckRegistration, HealthConfig, HealthRegistry, HealthRegistryError,
};
pub use scafra_foundation::{
    AuthorizationMode, AuthorizationPolicy, ControllerAuthorizationRegistration,
    RouteAuthorizationMetadata,
};
#[cfg(feature = "jwt")]
pub use scafra_security::JwtConfig;
#[cfg(feature = "security")]
pub use scafra_security::{BasicAuthConfig, SecurityConfig};
pub use server::{
    run, run_on, run_on_with_log_level, serve_application_router_with_options_and_callbacks,
    serve_finalized_listener_with_shutdown, serve_finalized_router_with_callbacks, serve_on,
    serve_on_with_policy, serve_on_with_shutdown, serve_router_with_options_and_callbacks,
    shutdown_channel, ApplicationRouter, ServerOutcome, ShutdownFuture, ShutdownHandle,
    ShutdownRequestError,
};
#[cfg(all(feature = "actuator", feature = "security"))]
pub use server::{
    serve_on_with_policy_and_actuator_and_security,
    serve_on_with_policy_and_actuator_and_security_and_request_timeout,
    serve_on_with_policy_and_actuator_and_security_and_request_timeout_and_callbacks,
    serve_on_with_policy_and_actuator_and_security_and_request_timeout_and_shutdown_callback,
    serve_router_on_with_policy_and_actuator_and_security_and_request_timeout,
    serve_router_on_with_policy_and_actuator_and_security_and_request_timeout_and_callbacks,
    serve_router_on_with_policy_and_actuator_and_security_and_request_timeout_and_shutdown_callback,
};

#[cfg(test)]
mod tests;

#[cfg(feature = "actuator")]
pub use server::{serve_on_with_actuator, serve_on_with_policy_and_actuator};
