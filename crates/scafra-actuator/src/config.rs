use serde::{Deserialize, Serialize};
use std::fmt;

/// Selects which built-in operational endpoint groups are installed.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EndpointSelection {
    /// A YAML list such as `['health', 'info']`.
    Names(Vec<String>),
    /// A properties-friendly value such as `health,info` or `*`.
    Pattern(String),
}

impl Default for EndpointSelection {
    fn default() -> Self {
        Self::Names(Vec::new())
    }
}

/// Controls which Scafra actuator endpoint groups are installed.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ActuatorConfig {
    /// Endpoint groups: `health`, `live`, `ready`, `info`, `metrics`, or `*` for all.
    #[serde(default)]
    pub endpoints: EndpointSelection,
    #[serde(default)]
    pub health: HealthConfig,
    #[serde(default)]
    pub security: ActuatorSecurity,
}

/// Controls how registered health checks are evaluated.
///
/// An empty `checks` list selects every check in the router's registry. A
/// `*` entry also selects every check. Unknown selected names are rejected by
/// graph-scoped router construction and [`crate::try_router`]; the legacy
/// [`crate::router`] reports `DOWN` for an unknown selection. Each selected
/// check runs sequentially and receives its own timeout.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthConfig {
    #[serde(default)]
    pub checks: Vec<String>,
    /// Maximum time allowed for each check, in milliseconds. Defaults to 1000;
    /// supported values are 1 through 300000, inclusive.
    #[serde(default = "default_check_timeout_ms")]
    pub check_timeout_ms: u64,
}

fn default_check_timeout_ms() -> u64 {
    1000
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            checks: Vec::new(),
            check_timeout_ms: default_check_timeout_ms(),
        }
    }
}

impl HealthConfig {
    /// Rejects zero and excessively long per-check deadlines before serving.
    pub fn validate(&self) -> Result<(), crate::HealthRegistryError> {
        if !(1..=300000).contains(&self.check_timeout_ms) {
            return Err(crate::HealthRegistryError::InvalidTimeout);
        }
        Ok(())
    }
}

/// Optional bearer-token protection for actuator endpoints.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct ActuatorSecurity {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing)]
    pub bearer_token: Option<String>,
}

impl fmt::Debug for ActuatorSecurity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActuatorSecurity")
            .field("enabled", &self.enabled)
            .field("bearer_token_configured", &self.bearer_token.is_some())
            .finish()
    }
}

impl ActuatorConfig {
    /// Validates endpoint names and the per-check deadline before installation.
    /// Credentials and invalid configured names are omitted from diagnostics.
    pub fn validate(&self) -> Result<(), crate::HealthRegistryError> {
        self.health.validate()?;
        if self.security.enabled
            && self
                .security
                .bearer_token
                .as_deref()
                .is_none_or(|token| token.trim().is_empty())
        {
            return Err(crate::HealthRegistryError::InvalidSecurityConfiguration);
        }
        let known =
            |name: &str| matches!(name, "*" | "health" | "live" | "ready" | "info" | "metrics");
        let valid = match &self.endpoints {
            EndpointSelection::Names(names) => names.iter().all(|name| known(name)),
            EndpointSelection::Pattern(pattern) => {
                pattern.is_empty() || pattern.split(',').all(|name| known(name.trim()))
            }
        };
        if !valid {
            return Err(crate::HealthRegistryError::InvalidEndpointSelection);
        }
        Ok(())
    }

    /// Enables every actuator endpoint group.
    pub fn all() -> Self {
        Self {
            endpoints: EndpointSelection::Pattern("*".to_owned()),
            health: HealthConfig::default(),
            security: ActuatorSecurity::default(),
        }
    }

    pub fn is_enabled(&self, endpoint: &str) -> bool {
        match &self.endpoints {
            EndpointSelection::Names(names) => {
                names.iter().any(|name| name == "*" || name == endpoint)
            }
            EndpointSelection::Pattern(pattern) => pattern
                .split(',')
                .map(str::trim)
                .any(|name| name == "*" || name == endpoint),
        }
    }

    /// Returns the enabled endpoint groups in stable display order.
    pub fn enabled_endpoints(&self) -> Vec<&'static str> {
        ["health", "live", "ready", "info", "metrics"]
            .into_iter()
            .filter(|endpoint| self.is_enabled(endpoint))
            .collect()
    }
}
