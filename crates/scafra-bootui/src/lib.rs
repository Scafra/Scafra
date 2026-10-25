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

/// Owned dashboard server started from the application's effective configuration.
/// Dropping the owner cancels its async serving task; synchronous user code is
/// cooperative and cannot be forcibly interrupted.
pub struct BootUiServer {
    shutdown: scafra_web::ShutdownHandle,
    task:
        Option<tokio::task::JoinHandle<Result<scafra_web::ServerOutcome, scafra_web::ServerError>>>,
}
impl BootUiServer {
    pub fn request_shutdown(&mut self) {
        let _ = self.shutdown.request();
    }
    pub async fn shutdown(mut self) -> Result<(), scafra_web::WebError> {
        self.request_shutdown();
        if let Some(task) = self.task.as_mut() {
            let result = task
                .await
                .map_err(|_| {
                    scafra_web::WebError::Server(std::io::Error::other(
                        "BootUI serving task failed",
                    ))
                })?
                .map(|_| ())
                .map_err(scafra_web::ServerError::into_web_error);
            self.task.take();
            result
        } else {
            Ok(())
        }
    }
}
impl Drop for BootUiServer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

#[allow(clippy::needless_update)] // Actuator is enabled only in some feature unions.
fn managed_router(
    config: &ScafraConfig,
    prepared: PreparedBootUi,
) -> Result<scafra_web::FinalizedRouter, BootUiPolicyError> {
    use std::time::Duration;
    let mut security = config.security.clone();
    // The outer context is authoritative. The dashboard must not inherit
    // application permit_all exceptions at either composition boundary.
    security.permit_all.clear();
    let options = scafra_web::RouterOptions {
        security,
        request_timeout: config
            .server
            .request_timeout_seconds
            .map(Duration::from_secs),
        max_request_body_bytes: config.server.max_request_body_bytes,
        connection: scafra_web::ConnectionOptions {
            header_read_timeout: Duration::from_secs(config.server.header_read_timeout_seconds),
            idle_timeout: Duration::from_secs(config.server.idle_timeout_seconds),
            max_connections: config.server.max_connections,
            max_header_bytes: config.server.max_header_bytes,
            http1_keep_alive: config.server.http1_keep_alive,
            http1_max_headers: config.server.http1_max_headers,
            http2_max_concurrent_streams: config.server.http2_max_concurrent_streams,
        },
        ..Default::default()
    };
    scafra_web::finish_managed_router(prepared.router, &[], &options)
        .map_err(|_| BootUiPolicyError::InvalidServerConfiguration)
}

/// Starts an owned dashboard using the same request, connection and drain
/// budgets as the main managed listener. The runner requests shutdown before
/// draining HTTP and joins this owner before releasing application resources.
pub fn start_managed_with_config(
    config: &ScafraConfig,
    profile: &str,
) -> Result<Option<BootUiServer>, BootUiPolicyError> {
    let Some(prepared) = prepare(config, profile)? else {
        return Ok(None);
    };
    let address = prepared.address;
    let router = managed_router(config, prepared)?;
    let policy = config.shutdown.policy();
    let (shutdown, signal) = scafra_web::shutdown_channel();
    let task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(address)
            .await
            .map_err(|source| {
                tracing::error!(error_kind = "bootui_bind", "BootUI listener could not bind");
                scafra_web::ServerError::from(scafra_web::WebError::Bind { address, source })
            })?;
        tracing::info!(target: "scafra::startup", %address, "Scafra BootUI started");
        scafra_web::serve_finalized_listener_with_shutdown(listener, router, signal, policy).await
    });
    Ok(Some(BootUiServer {
        shutdown,
        task: Some(task),
    }))
}

/// Starts BootUI from Scafra's already resolved application configuration.
///
/// Compatibility API for independently started dashboards. The standard runner
/// uses [`start_managed_with_config`] and owns its shutdown. The snapshot is created from the same validated configuration and active
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

    let address = prepared.address;
    let router = match managed_router(config, prepared) {
        Ok(router) => router,
        Err(_) => {
            tracing::error!(
                error_kind = "bootui_server_configuration",
                "BootUI server configuration rejected"
            );
            return;
        }
    };
    let policy = config.shutdown.policy();
    tokio::spawn(async move {
        if scafra_web::serve_finalized_router_with_callbacks(
            address,
            policy,
            router,
            (|_| {}, || {}),
        )
        .await
        .is_err()
        {
            tracing::error!(
                error_kind = "bootui_server",
                "BootUI server stopped with an error"
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
    config
        .bootui
        .validate_path()
        .map_err(|_| BootUiPolicyError::InvalidPath)?;
    Ok(router.nest(path, dashboard))
}

struct PreparedBootUi {
    address: SocketAddr,
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
    config
        .bootui
        .validate_path()
        .map_err(|_| BootUiPolicyError::InvalidPath)?;

    let dashboard = dashboard::router(
        snapshot::BootUiSnapshot::from_config(config, profile),
        binding.protection,
    );
    Ok(Some(PreparedBootUi {
        address: binding.address,
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
