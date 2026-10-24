//! Modular Scafra facade. Select capabilities using Cargo features.

pub use scafra_config as config;
pub use scafra_config::{
    BootUiConfig, Config, ConfigCapabilities, ConfigError, ConfigLoader, ConfigProperties,
    LoggingConfig, Properties, ScafraConfig, ServerConfig, StartupConfig, ValidationError,
};
pub use scafra_core as core;
pub use scafra_core::{
    Application, ApplicationContext, AsyncResource, AsyncResourceSet, Bean, BeanPostProcessor,
    ComponentKind, ComponentMetadata, GraphDescriptor, GraphEdgeDescriptor, GraphError,
    GraphNodeDescriptor, GraphNodeKind, GraphPhase, GraphPlan, ProviderFailure, ScafraError,
};
pub use scafra_foundation as foundation;
pub use scafra_foundation::{
    startup, AuthorizationMode, AuthorizationPolicy, BacktraceMode, LogLevel, ShutdownPolicy,
    ShutdownReason,
};
pub use scafra_macros::{
    bean, component, controller, delete, get, logger, main, post, post_processor, put, repository,
    routes, service, Config,
};
pub use tracing::{debug, error, info, trace, warn};

#[cfg(feature = "actuator")]
pub use scafra_actuator as actuator;
#[cfg(feature = "actuator")]
pub use scafra_actuator::{
    register_health_check, ActuatorConfig, ActuatorSecurity, EndpointSelection, HealthConfig,
    HealthRegistry, HealthRegistryError,
};
#[cfg(feature = "bootui")]
pub use scafra_bootui as bootui;
#[cfg(feature = "scheduler")]
pub use scafra_scheduler::{
    register_scheduled_task, ScheduledTaskConfig, ScheduledTaskRegistration, Scheduler,
    SchedulerConfig, SchedulerConfigError, SchedulerError, SchedulerMetrics,
    SchedulerMetricsSnapshot, SchedulerShutdownHandle,
};
#[cfg(feature = "security")]
pub use scafra_security::{BasicAuthConfig, JwtConfig, SecurityConfig};
#[cfg(feature = "web")]
pub use scafra_web as web;
#[cfg(feature = "web")]
pub use scafra_web::{
    build_router, join_paths, run_on, run_on_with_log_level, serve_on, serve_on_with_policy,
    serve_on_with_shutdown, shutdown_channel, AppError, AuthenticationChallenge, ControllerPrefix,
    ControllerRegistration, ControllerRoutes, JsonBody, RouteAuthorizationMetadata, RouteMetadata,
    ServerError, ServerOutcome, ShutdownFuture, ShutdownHandle, ShutdownRequestError, WebError,
};
#[cfg(feature = "actuator")]
pub use scafra_web::{
    build_router_with_actuator_and_health_registry, serve_on_with_actuator,
    serve_on_with_policy_and_actuator,
};
#[cfg(all(feature = "actuator", feature = "security"))]
pub use scafra_web::{
    build_router_with_actuator_and_health_registry_and_security,
    serve_on_with_policy_and_actuator_and_security,
    serve_on_with_policy_and_actuator_and_security_and_request_timeout_and_shutdown_callback,
    serve_router_on_with_policy_and_actuator_and_security_and_request_timeout_and_shutdown_callback,
};
#[cfg(feature = "web")]
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(feature = "web")]
mod startup;
#[cfg(feature = "web")]
pub use startup::{
    router_options, run, run_with_finalized_resources, run_with_finalized_router,
    run_with_resources, run_with_router,
};
#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
mod feature_macros;
pub mod prelude;

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    // Keep the payload for compatibility with callers that inspect the
    // variant, but never expose the configured value through Display. The
    // address may have come from an environment variable or secret-bearing
    // configuration source.
    #[error("invalid configured server address")]
    Address(String),

    #[error(transparent)]
    Core(#[from] scafra_core::ScafraError),

    #[error(transparent)]
    Graph(#[from] scafra_core::GraphError),

    #[cfg(feature = "scheduler")]
    #[error(transparent)]
    Scheduler(#[from] scafra_scheduler::SchedulerError),

    #[cfg(feature = "web")]
    #[error(transparent)]
    Web(#[from] WebError),
}

impl StartupError {
    /// Returns the stable public category used by generated startup logs.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Config(_) => "configuration",
            Self::Address(_) => "address",
            Self::Core(_) => "lifecycle",
            Self::Graph(_) => "dependency_graph",
            #[cfg(feature = "scheduler")]
            Self::Scheduler(_) => "scheduler",
            #[cfg(feature = "web")]
            Self::Web(_) => "web",
        }
    }
}
