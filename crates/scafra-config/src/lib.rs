//! Typed configuration primitives.

mod errors;
mod extensions;
mod merge;
mod models;
mod sources;

pub use errors::ConfigError;
#[doc(hidden)]
pub use extensions::ConfiguredExtensionRegistration;
pub use models::{BootUiConfig, ScafraConfig, ServerConfig, StartupConfig, ValidationError};
pub use scafra_foundation::{BacktraceMode, LogLevel, LoggingConfig};
#[cfg(feature = "scheduler")]
pub use scafra_scheduler::{SchedulerConfig, SchedulerConfigError};
#[cfg(feature = "security")]
pub use scafra_security::{JwtConfig, SecurityConfig};
pub use sources::{
    load_yaml, Config, ConfigCapabilities, ConfigLoader, ConfigProperties, Properties,
};

#[cfg(test)]
pub(crate) use merge::validate_profile;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_defaults_are_local_and_predictable() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn profile_validation_rejects_path_components_without_echoing_them() {
        for profile in ["", ".", "..", "../secret", r"nested\secret"] {
            let error = validate_profile(Some(profile)).expect_err("profile should be rejected");
            assert_eq!(error.to_string(), "invalid configuration profile");
            if !profile.is_empty() {
                assert!(!error.to_string().contains(profile));
            }
        }
    }
}
