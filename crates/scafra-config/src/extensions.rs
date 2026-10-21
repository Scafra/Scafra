use crate::ScafraConfig;

/// A configuration-aware startup extension used by the Scafra runner.
///
/// This registration is invoked after the application listener binds and
/// receives the already resolved, validated configuration and profile.
#[doc(hidden)]
pub struct ConfiguredExtensionRegistration {
    pub name: &'static str,
    pub start: fn(&ScafraConfig, &str),
}

inventory::collect!(ConfiguredExtensionRegistration);
