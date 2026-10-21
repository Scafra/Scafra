//! Optional local operations dashboard for Scafra applications.
//!
//! Add this crate explicitly when a browser-based operations view is wanted.
//! Scafra core does not install or expose the dashboard by default.

mod binding;
mod dashboard;
mod snapshot;

pub use binding::BootUiPolicyError;
pub use scafra_config::BootUiConfig;
use scafra_config::{ConfigLoader, ScafraConfig};
use std::net::SocketAddr;

inventory::submit! {
    scafra_config::ConfiguredExtensionRegistration {
        name: "scafra-bootui",
        start: start_with_config,
    }
}

/// Starts BootUI from Scafra's already resolved application configuration.
///
/// The standard runner invokes this after the application listener binds. The
/// snapshot is created from the same validated configuration and active
/// profile used by the application.
/// Invalid or unsafe bind policies are logged and prevent the listener from
/// starting.
pub fn start_with_config(config: &ScafraConfig, profile: &str) {
    let prepared = match prepare(config, profile) {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return,
        Err(error) => {
            tracing::error!(
                error_kind = "bootui_policy",
                %error,
                "BootUI was not started; correct the BootUI bind and security configuration"
            );
            return;
        }
    };

    tokio::spawn(async move {
        let address = prepared.address;
        let path = prepared.path.clone();
        let listener = match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(
                    %address,
                    error_kind = "bootui_bind",
                    "BootUI could not bind its configured address: {error}"
                );
                return;
            }
        };
        tracing::info!(target: "scafra::startup", %address, path = %path, "Scafra BootUI started");
        let service = prepared
            .router
            .into_make_service_with_connect_info::<SocketAddr>();
        if let Err(error) = axum::serve(listener, service).await {
            tracing::error!(
                error_kind = "bootui_server",
                "BootUI stopped unexpectedly: {error}"
            );
        }
    });
}

/// Starts BootUI manually using the configuration selected by the standard
/// loader. Applications using the Scafra runner should let the runner start it
/// so it receives the effective in-memory configuration directly.
pub fn start() {
    let (config, profile) = match ConfigLoader::new().load_validated_with_profile::<ScafraConfig>()
    {
        Ok(config) => config,
        Err(_) => {
            tracing::error!(
                error_kind = "bootui_configuration",
                "BootUI configuration could not be loaded"
            );
            return;
        }
    };
    start_with_config(&config, &profile);
}

/// Adds the BootUI dashboard to an existing Scafra router.
///
/// For compatibility, the dashboard settings are supplied separately and the
/// remaining application settings are loaded once from the normal
/// configuration sources. Prefer [`try_layer_with_config`] when the
/// application already has its complete configuration value.
///
/// A local-only dashboard checks the connecting peer address and rejects
/// requests when the server does not provide connection information. A
/// non-local dashboard is added only when Scafra authentication is configured.
/// Use Scafra's server adapter, which supplies peer information, or provide
/// equivalent `ConnectInfo<SocketAddr>` extensions in a custom Axum server.
pub fn layer(router: axum::Router, config: &BootUiConfig) -> axum::Router {
    if !config.enabled {
        return router;
    }

    let (mut application_config, profile) =
        match ConfigLoader::new().load_validated_with_profile::<ScafraConfig>() {
            Ok(config) => config,
            Err(_) => {
                tracing::warn!(
                error_kind = "bootui_configuration",
                "BootUI routes were not added because application configuration could not be loaded"
            );
                return router;
            }
        };
    application_config.bootui = config.clone();
    match try_layer_with_config_and_profile(router.clone(), &application_config, &profile) {
        Ok(router) => router,
        Err(error) => {
            tracing::error!(
                error_kind = "bootui_policy",
                %error,
                "BootUI routes were not added; correct the BootUI bind and security configuration"
            );
            router
        }
    }
}

/// Adds BootUI using a complete application configuration value.
///
/// This checked API is useful for custom configuration sources and returns a
/// policy error instead of silently omitting the dashboard. When
/// `local_only=true`, the configured BootUI address must be loopback and
/// requests from non-loopback peers are rejected. When `local_only=false`,
/// Scafra authentication must be enabled and is applied directly to every
/// BootUI route, regardless of `security.permit_all`.
pub fn try_layer_with_config(
    router: axum::Router,
    config: &ScafraConfig,
) -> Result<axum::Router, BootUiPolicyError> {
    try_layer_with_config_and_profile(router, config, "default")
}

/// Adds BootUI using the effective application configuration and selected
/// profile. The profile is included in the allow-listed dashboard snapshot.
pub fn try_layer_with_config_and_profile(
    router: axum::Router,
    config: &ScafraConfig,
    profile: &str,
) -> Result<axum::Router, BootUiPolicyError> {
    if !config.bootui.enabled {
        return Ok(router);
    }

    let binding = binding::resolve(&config.bootui, &config.security)?;
    let snapshot = snapshot::BootUiSnapshot::from_config(config, profile);
    let dashboard = dashboard::router(snapshot, binding.protection);
    let path = config.bootui.path.trim_end_matches('/');
    if path.is_empty() || !path.starts_with('/') {
        return Err(BootUiPolicyError::InvalidPath);
    }
    Ok(router.nest(path, dashboard))
}

struct PreparedBootUi {
    address: SocketAddr,
    path: String,
    router: axum::Router,
}

fn prepare(
    config: &ScafraConfig,
    profile: &str,
) -> Result<Option<PreparedBootUi>, BootUiPolicyError> {
    if !config.bootui.enabled {
        return Ok(None);
    }

    let binding = binding::resolve(&config.bootui, &config.security)?;
    let path = config.bootui.path.trim_end_matches('/');
    if path.is_empty() || !path.starts_with('/') {
        return Err(BootUiPolicyError::InvalidPath);
    }

    let dashboard = dashboard::router(
        snapshot::BootUiSnapshot::from_config(config, profile),
        binding.protection,
    );
    Ok(Some(PreparedBootUi {
        address: binding.address,
        path: path.to_owned(),
        router: axum::Router::new().nest(path, dashboard),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_startup_policy_fails_before_a_listener_can_be_created() {
        let mut config = ScafraConfig::default();
        config.bootui.enabled = true;
        config.bootui.host = "0.0.0.0".to_owned();
        config.bootui.local_only = true;

        assert!(matches!(
            prepare(&config, "default"),
            Err(BootUiPolicyError::LoopbackRequired)
        ));
    }

    #[test]
    fn startup_policy_accepts_ipv4_and_ipv6_loopback_addresses() {
        for host in ["127.0.0.1", "::1"] {
            let mut config = ScafraConfig::default();
            config.bootui.enabled = true;
            config.bootui.host = host.to_owned();

            let prepared = match prepare(&config, "default") {
                Ok(Some(prepared)) => prepared,
                Ok(None) => panic!("enabled BootUI should be prepared"),
                Err(error) => panic!("loopback host {host} should be accepted: {error}"),
            };
            assert!(prepared.address.ip().is_loopback());
        }
    }
}
