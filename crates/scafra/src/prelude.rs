//! Capability-aware convenient imports.
#[cfg(feature = "web")]
pub use crate::web::axum::{
    extract::{Json, Path, Query},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
pub use crate::{
    bean, component, debug, error, info, logger, post_processor, repository, service, startup,
    trace, warn, Application, ApplicationContext, AsyncResource, AsyncResourceSet,
    AuthorizationMode, AuthorizationPolicy, BacktraceMode, Bean, BeanPostProcessor, BootUiConfig,
    ComponentKind, ComponentMetadata, Config, ConfigCapabilities, ConfigError, ConfigLoader,
    ConfigProperties, GraphDescriptor, GraphEdgeDescriptor, GraphError, GraphNodeDescriptor,
    GraphNodeKind, GraphPhase, GraphPlan, LogLevel, LoggingConfig, Properties, ProviderFailure,
    ScafraConfig, ScafraError, ServerConfig, ShutdownConfig, ShutdownPolicy, ShutdownReason,
    StartupConfig, StartupError, ValidationError,
};
#[cfg(feature = "web")]
pub use crate::{
    build_router, controller, delete, get, main, post, put, routes, AppError,
    AuthenticationChallenge, ControllerPrefix, ControllerRegistration, ControllerRoutes, JsonBody,
    Result, RouteMetadata, ServerError, ServerOutcome, WebError,
};
#[cfg(feature = "actuator")]
pub use crate::{register_health_check, ActuatorConfig, HealthRegistry, HealthRegistryError};
#[cfg(feature = "scheduler")]
pub use crate::{
    register_scheduled_task, ScheduledTaskConfig, ScheduledTaskRegistration, Scheduler,
    SchedulerConfig, SchedulerConfigError, SchedulerError, SchedulerMetrics,
    SchedulerMetricsSnapshot, SchedulerShutdownHandle,
};
