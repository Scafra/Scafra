use crate::sources::Config;
#[cfg(feature = "actuator")]
use scafra_actuator::ActuatorConfig;
use scafra_foundation::LoggingConfig;
#[cfg(feature = "scheduler")]
use scafra_scheduler::SchedulerConfig;
#[cfg(feature = "security")]
use scafra_security::SecurityConfig;
/// HTTP listener and resource policies resolved once during startup.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// Handler deadline in seconds. Explicit `null` disables it; omission uses 30 seconds.
    #[serde(rename = "timeout", alias = "request_timeout_seconds")]
    pub request_timeout_seconds: Option<u64>,
    /// Global hard ceiling, including raw and streamed request bodies.
    pub max_request_body_bytes: usize,
    pub header_read_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub max_connections: usize,
    pub max_header_bytes: usize,
    pub http1_keep_alive: bool,
    pub http1_max_headers: usize,
    pub http2_max_concurrent_streams: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            request_timeout_seconds: Some(30),
            max_request_body_bytes: 1024 * 1024,
            header_read_timeout_seconds: 10,
            idle_timeout_seconds: 60,
            max_connections: 1024,
            max_header_bytes: 32768,
            http1_keep_alive: true,
            http1_max_headers: 100,
            http2_max_concurrent_streams: 200,
        }
    }
}

impl Config for ServerConfig {
    type Error = ValidationError;

    fn validate(&self) -> Result<(), Self::Error> {
        if self.host.trim().is_empty() {
            return Err(invalid("server.host", "must not be empty"));
        }
        if self.host.parse::<std::net::IpAddr>().is_err() {
            return Err(invalid(
                "server.host",
                "must be an IPv4 or IPv6 address literal",
            ));
        }
        if self.port == 0 {
            return Err(invalid("server.port", "must be greater than zero"));
        }
        if self
            .request_timeout_seconds
            .is_some_and(|seconds| !(1..=86400).contains(&seconds))
        {
            return Err(invalid(
                "server.request_timeout_seconds",
                "must be between 1 and 86400 seconds, or null to disable",
            ));
        }
        for (field, value, minimum, maximum) in [
            (
                "server.max_request_body_bytes",
                self.max_request_body_bytes as u64,
                1,
                1073741824,
            ),
            (
                "server.header_read_timeout_seconds",
                self.header_read_timeout_seconds,
                1,
                3600,
            ),
            (
                "server.idle_timeout_seconds",
                self.idle_timeout_seconds,
                1,
                86400,
            ),
            (
                "server.max_connections",
                self.max_connections as u64,
                1,
                1000000,
            ),
            (
                "server.max_header_bytes",
                self.max_header_bytes as u64,
                8192,
                1048576,
            ),
            (
                "server.http1_max_headers",
                self.http1_max_headers as u64,
                1,
                1000,
            ),
            (
                "server.http2_max_concurrent_streams",
                u64::from(self.http2_max_concurrent_streams),
                1,
                10000,
            ),
        ] {
            if !(minimum..=maximum).contains(&value) {
                return Err(invalid(
                    field,
                    &format!("must be between {minimum} and {maximum}"),
                ));
            }
        }
        Ok(())
    }

    fn validation_details(error: &Self::Error) -> Option<ValidationError> {
        Some(error.clone())
    }
}

/// Timing contract shared by managed HTTP draining and resource cleanup.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShutdownConfig {
    pub grace_period_seconds: u64,
    /// Stop waiting for cooperative async work after the grace period.
    /// This cannot interrupt synchronous callbacks or undo external effects.
    pub force_after_grace: bool,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            grace_period_seconds: 30,
            force_after_grace: true,
        }
    }
}

impl ShutdownConfig {
    pub fn policy(&self) -> scafra_foundation::ShutdownPolicy {
        scafra_foundation::ShutdownPolicy::new(
            std::time::Duration::from_secs(self.grace_period_seconds),
            self.force_after_grace,
        )
    }
}

impl Config for ShutdownConfig {
    type Error = ValidationError;

    fn validate(&self) -> Result<(), Self::Error> {
        if !(1..=86400).contains(&self.grace_period_seconds) {
            return Err(invalid(
                "shutdown.grace_period_seconds",
                "must be between 1 and 86400 seconds",
            ));
        }
        Ok(())
    }

    fn validation_details(error: &Self::Error) -> Option<ValidationError> {
        Some(error.clone())
    }
}

fn invalid(field: &'static str, message: &str) -> ValidationError {
    ValidationError {
        field,
        message: message.to_owned(),
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("configuration validation failed for {field}: {message}")]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct StartupConfig {
    #[serde(default = "default_true")]
    pub banner: bool,
    #[serde(default = "default_true")]
    pub show_config: bool,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            banner: true,
            show_config: true,
        }
    }
}

/// Configuration reserved for the optional `scafra-bootui` dependency.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BootUiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bootui_host")]
    pub host: String,
    #[serde(default = "default_bootui_port")]
    pub port: u16,
    #[serde(default = "default_bootui_path")]
    pub path: String,
    #[serde(default = "default_true")]
    pub local_only: bool,
}

impl Default for BootUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_bootui_host(),
            port: default_bootui_port(),
            path: default_bootui_path(),
            local_only: true,
        }
    }
}

impl Config for BootUiConfig {
    type Error = ValidationError;

    fn validate(&self) -> Result<(), Self::Error> {
        if self.port == 0 {
            return Err(invalid("bootui.port", "must be greater than zero"));
        }
        self.validate_path()?;
        if self.enabled {
            let ip = self
                .host
                .parse::<std::net::IpAddr>()
                .map_err(|_| invalid("bootui.host", "must be an IPv4 or IPv6 address literal"))?;
            if self.local_only && !ip.is_loopback() {
                return Err(invalid(
                    "bootui.host",
                    "must be loopback when local_only is true",
                ));
            }
        }
        Ok(())
    }

    fn validation_details(error: &Self::Error) -> Option<ValidationError> {
        Some(error.clone())
    }
}

impl BootUiConfig {
    /// Dashboard mounts are static non-root absolute paths, never Axum patterns.
    pub fn validate_path(&self) -> Result<(), ValidationError> {
        let path = self.path.trim_end_matches('/');
        if path.is_empty()
            || !path.starts_with('/')
            || path.chars().any(|character| {
                character.is_control()
                    || matches!(character, '{' | '}' | '*' | ':' | '?' | '#' | '\\')
            })
        {
            return Err(invalid(
                "bootui.path",
                "must be a non-root static absolute path",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ScafraConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub shutdown: ShutdownConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[cfg(feature = "actuator")]
    #[serde(default)]
    pub actuator: ActuatorConfig,
    #[serde(default)]
    pub startup: StartupConfig,
    #[cfg(feature = "scheduler")]
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[cfg(feature = "security")]
    #[serde(default)]
    pub security: SecurityConfig,
    #[cfg(feature = "bootui")]
    #[serde(default)]
    pub bootui: BootUiConfig,
}

impl Config for ScafraConfig {
    type Error = ValidationError;

    const FRAMEWORK_CONFIGURATION: bool = true;

    fn validate(&self) -> Result<(), Self::Error> {
        self.server.validate()?;
        self.shutdown.validate()?;
        #[cfg(feature = "scheduler")]
        if let Err(error) = self.scheduler.validate() {
            let (field, message) = match error {
                scafra_scheduler::SchedulerConfigError::UnknownTask => (
                    "scheduler.tasks",
                    "contains a task that is not registered".to_owned(),
                ),
                scafra_scheduler::SchedulerConfigError::ZeroInterval => (
                    "scheduler.tasks.interval_ms",
                    "must be greater than zero milliseconds".to_owned(),
                ),
                scafra_scheduler::SchedulerConfigError::IntervalTooLarge => (
                    "scheduler.tasks.interval_ms",
                    "must not exceed one year".to_owned(),
                ),
                scafra_scheduler::SchedulerConfigError::InvalidRegistration => (
                    "scheduler",
                    "contains a task registration with an invalid name or interval".to_owned(),
                ),
                scafra_scheduler::SchedulerConfigError::DuplicateRegistration => (
                    "scheduler",
                    "contains duplicate task registrations".to_owned(),
                ),
            };
            return Err(ValidationError { field, message });
        }
        #[cfg(feature = "actuator")]
        if let Err(error) = self.actuator.validate() {
            let (field, message) = match error {
                scafra_actuator::HealthRegistryError::InvalidSecurityConfiguration => (
                    "actuator.security.bearer_token",
                    "must be a non-blank bearer token when actuator security is enabled",
                ),
                scafra_actuator::HealthRegistryError::InvalidEndpointSelection => (
                    "actuator.endpoints",
                    "must contain only supported endpoint names",
                ),
                _ => (
                    "actuator.health.check_timeout_ms",
                    "must be between 1 and 300000 milliseconds",
                ),
            };
            return Err(invalid(field, message));
        }
        #[cfg(feature = "security")]
        if let Err(reason) = self.security.validate() {
            return Err(security_validation_error(reason));
        }
        #[cfg(feature = "bootui")]
        {
            self.bootui.validate()?;
            if self.bootui.enabled && !self.bootui.local_only {
                #[cfg(feature = "security")]
                let authenticated = self.security.enabled;
                #[cfg(not(feature = "security"))]
                let authenticated = false;
                if !authenticated {
                    return Err(invalid(
                        "bootui.local_only",
                        "false requires enabled security with valid credentials",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validation_details(error: &Self::Error) -> Option<ValidationError> {
        Some(error.clone())
    }
}

#[cfg(feature = "security")]
fn security_validation_error(reason: &'static str) -> ValidationError {
    let (field, message) = match reason {
        "basic authentication requires both username and password" => (
            "security.basic", "requires both username and password",
        ),
        "JWT configuration requires the `jwt` feature" => {
            ("security.jwt", "requires the jwt Cargo feature")
        }
        "remote JWT configuration requires the `jwks` feature" => {
            ("security.jwt", "requires the jwks Cargo feature")
        }
        "JWT leeway_seconds must be between 0 and 300 seconds" => (
            "security.jwt.leeway_seconds", "must be between 0 and 300 seconds",
        ),
        "JWT cache_ttl_seconds must be between 1 and 86400 seconds" => (
            "security.jwt.cache_ttl_seconds", "must be between 1 and 86400 seconds",
        ),
        "JWT refresh_cooldown_seconds must be between 1 and 3600 seconds and not exceed cache_ttl_seconds" => (
            "security.jwt.refresh_cooldown_seconds", "must be between 1 and 3600 seconds and not exceed cache_ttl_seconds",
        ),
        "JWT connect_timeout_ms must be between 1 and 300000 milliseconds" => (
            "security.jwt.connect_timeout_ms", "must be between 1 and 300000 milliseconds",
        ),
        "JWT request_timeout_ms must be between 1 and 300000 milliseconds and not be less than connect_timeout_ms" => (
            "security.jwt.request_timeout_ms", "must be between 1 and 300000 milliseconds and not be less than connect_timeout_ms",
        ),
        "the bearer token must not be empty or whitespace-only" => (
            "security.bearer_token",
            "must not be empty or whitespace-only",
        ),
        "JWT requires secret, issuer_uri, or jwk_set_uri" => (
            "security.jwt",
            "requires secret, issuer_uri, or jwk_set_uri",
        ),
        "the JWT secret must not be empty" => (
            "security.jwt.secret",
            "must not be empty or whitespace-only",
        ),
        "JWT secret cannot be combined with remote JWKS configuration" => (
            "security.jwt",
            "secret cannot be combined with remote JWKS configuration",
        ),
        "JWT issuer_uri must be a valid HTTP(S) URL" => {
            ("security.jwt.issuer_uri", "must be a valid HTTP(S) URL")
        }
        "JWT jwk_set_uri must be a valid HTTP(S) URL" => {
            ("security.jwt.jwk_set_uri", "must be a valid HTTP(S) URL")
        }
        "JWT required_scopes must not contain empty or whitespace-only values" => (
            "security.jwt.required_scopes",
            "must not contain empty or whitespace-only values",
        ),
        "JWT audiences must not contain empty or whitespace-only values" => (
            "security.jwt.audiences",
            "must not contain empty or whitespace-only values",
        ),
        _ => (
            "security",
            "credentials must be configured when security is enabled",
        ),
    };
    ValidationError {
        field,
        message: message.to_owned(),
    }
}

fn default_host() -> String {
    "127.0.0.1".to_owned()
}

fn default_port() -> u16 {
    8080
}

fn default_true() -> bool {
    true
}

fn default_bootui_host() -> String {
    "127.0.0.1".to_owned()
}

fn default_bootui_port() -> u16 {
    8091
}

fn default_bootui_path() -> String {
    "/bootui".to_owned()
}
