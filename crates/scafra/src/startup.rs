use crate::*;
use std::{future::Future, io::IsTerminal, sync::Arc};

fn server_shutdown_reason(error: &ServerError) -> ShutdownReason {
    match error {
        ServerError::GracefulShutdownTimeout { reason, .. } => *reason,
        ServerError::Web(WebError::DuplicateRoute { .. } | WebError::Bind { .. }) => {
            ShutdownReason::StartupFailure
        }
        ServerError::Web(WebError::Server(source))
            if source.kind() == std::io::ErrorKind::InvalidInput =>
        {
            ShutdownReason::StartupFailure
        }
        ServerError::Web(WebError::Server(_)) => ShutdownReason::RuntimeFailure,
    }
}

/// Starts the standard Scafra server using the default local address.
pub async fn run() -> std::result::Result<(), StartupError> {
    run_inner(|_| async { Ok((None, ())) }).await
}

/// Starts Scafra with a router composed from the application's typed graph.
/// The builder runs after validated configuration loads and before lifecycle
/// startup, so providers can use configured values without global state.
/// The server finalizes linked controller policies before binding, even when
/// the supplied router was assembled using low-level registration adapters.
/// Generated route-local guards preserve roles/scopes after nesting. This legacy
/// boundary also applies global security; use `run_with_finalized_router` and
/// `web::finish_managed_router` for nested explicit public overrides without
/// reinstalling middleware.
pub async fn run_with_router<F>(build_router: F) -> std::result::Result<(), StartupError>
where
    F: FnOnce(&ScafraConfig) -> std::result::Result<axum::Router, StartupError>,
{
    run_inner(|config| {
        let result =
            build_router(&config).map(|router| (Some(web::ApplicationRouter::Raw(router)), ()));
        async move { result }
    })
    .await
}

/// Starts Scafra after asynchronously composing a router and its typed resource
/// set. The set remains owned by the application until HTTP requests, the
/// scheduler, and lifecycle hooks have shut down.
pub async fn run_with_resources<F, Fut, R>(
    build_application: F,
) -> std::result::Result<(), StartupError>
where
    F: FnOnce(ScafraConfig) -> Fut,
    Fut: Future<Output = std::result::Result<(axum::Router, R), StartupError>>,
    R: AsyncResourceSet,
{
    run_inner(|config| async move {
        build_application(config)
            .await
            .map(|(router, resources)| (Some(web::ApplicationRouter::Raw(router)), resources))
    })
    .await
}

/// Runs an explicitly finalized composition without reinstalling middleware.
/// Use `web::finish_managed_router` to support nested controller public overrides.
pub async fn run_with_finalized_router<F>(build_router: F) -> std::result::Result<(), StartupError>
where
    F: FnOnce(&ScafraConfig) -> std::result::Result<web::FinalizedRouter, StartupError>,
{
    run_inner(|config| {
        let result = build_router(&config)
            .map(|router| (Some(web::ApplicationRouter::Finalized(router)), ()));
        async move { result }
    })
    .await
}

/// Runs finalized typed graph composition while retaining managed resource owners.
pub async fn run_with_finalized_resources<F, Fut, R>(
    build_application: F,
) -> std::result::Result<(), StartupError>
where
    F: FnOnce(ScafraConfig) -> Fut,
    Fut: Future<Output = std::result::Result<(web::FinalizedRouter, R), StartupError>>,
    R: AsyncResourceSet,
{
    run_inner(|config| async move {
        build_application(config)
            .await
            .map(|(router, resources)| (Some(web::ApplicationRouter::Finalized(router)), resources))
    })
    .await
}

async fn run_inner<F, Fut, R>(build_runtime: F) -> std::result::Result<(), StartupError>
where
    F: FnOnce(ScafraConfig) -> Fut,
    Fut: Future<Output = std::result::Result<(Option<web::ApplicationRouter>, R), StartupError>>,
    R: AsyncResourceSet,
{
    let loader = ConfigLoader::new().capabilities(config::ConfigCapabilities {
        security: cfg!(feature = "security"),
        jwt: cfg!(feature = "jwt"),
        jwks: cfg!(feature = "jwks"),
        actuator: cfg!(feature = "actuator"),
        scheduler: cfg!(feature = "scheduler"),
        bootui: cfg!(feature = "bootui"),
    });
    let (config, profile) = loader.load_validated_with_profile::<ScafraConfig>()?;
    scafra_foundation::init_runtime_logging(&config.logging);
    startup!(profile = %profile, root = %loader.configuration_root().display(), "configuration loaded");
    startup!(level = %config.logging.level, "logging initialized");
    #[cfg(feature = "bootui")]
    {
        let bootui_installed = inventory::iter::<scafra_config::ConfiguredExtensionRegistration>()
            .any(|extension| extension.name == "scafra-bootui");
        if config.bootui.enabled && !bootui_installed {
            scafra_foundation::warn!(
            "BootUI is enabled in configuration, but the scafra-bootui dependency is not installed"
        );
        } else {
            startup!(
                installed = bootui_installed,
                enabled = config.bootui.enabled,
                bind = %format!("{}:{}", config.bootui.host, config.bootui.port),
                path = %config.bootui.path,
                "BootUI status"
            );
        }
    }
    if config.startup.banner {
        print_startup_banner(&config, &loader, &profile);
    }
    let address = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|error: std::net::AddrParseError| StartupError::Address(error.to_string()))?;
    let (router, resources) = build_runtime(config.clone()).await?;
    let context = ApplicationContext::discover();
    startup!(
        components = context.metadata().len(),
        "application components discovered"
    );
    let mut application = Application::new(context);
    if let Err(error) = application.start() {
        resources.request_shutdown(ShutdownReason::StartupFailure);
        if let Err(cleanup) = resources
            .shutdown(ShutdownReason::StartupFailure, ShutdownPolicy::default())
            .await
        {
            __private::log_cleanup_failure(&StartupError::from(cleanup));
        }
        return Err(StartupError::from(error));
    }
    startup!("application lifecycle started");
    let policy = ShutdownPolicy::default();
    #[cfg(feature = "scheduler")]
    let scheduler = match Scheduler::try_start(&config.scheduler) {
        Ok(scheduler) => scheduler,
        Err(error) => {
            let error = StartupError::from(error);
            if let Err(cleanup) = application.shutdown_with(ShutdownReason::StartupFailure, policy)
            {
                __private::log_cleanup_failure(&StartupError::from(cleanup));
            }
            resources.request_shutdown(ShutdownReason::StartupFailure);
            if let Err(cleanup) = resources
                .shutdown(ShutdownReason::StartupFailure, policy)
                .await
            {
                __private::log_cleanup_failure(&StartupError::from(cleanup));
            }
            return Err(error);
        }
    };
    #[cfg(feature = "scheduler")]
    startup!(enabled = config.scheduler.enabled, "scheduler initialized");
    let resources = Arc::new(resources);
    #[cfg(feature = "scheduler")]
    let scheduler_shutdown = scheduler.shutdown_handle();
    let resources_shutdown = Arc::clone(&resources);
    let on_shutdown_requested = move |reason| {
        #[cfg(feature = "scheduler")]
        scheduler_shutdown.request_shutdown();
        resources_shutdown.request_shutdown(reason);
    };
    let extension_config = config.clone();
    let extension_profile = profile.clone();
    let on_server_started = move || {
        let mut extensions =
            inventory::iter::<scafra_config::ConfiguredExtensionRegistration>().collect::<Vec<_>>();
        extensions.sort_by_key(|extension| extension.name);
        for extension in extensions {
            (extension.start)(&extension_config, &extension_profile);
        }
    };
    let options = router_options(&config);
    let (result, reason) = match scafra_web::serve_application_router_with_options_and_callbacks(
        address,
        policy,
        options,
        router,
        (on_shutdown_requested, on_server_started),
    )
    .await
    {
        Ok(outcome) => (Ok(()), outcome.reason()),
        Err(error) => {
            let reason = server_shutdown_reason(&error);
            (Err(StartupError::from(error.into_web_error())), reason)
        }
    };
    #[cfg(feature = "scheduler")]
    scheduler.shutdown().await;
    resources.request_shutdown(reason);
    let shutdown = application
        .shutdown_with(reason, policy)
        .map_err(StartupError::from);
    let resource_shutdown = resources
        .shutdown(reason, policy)
        .await
        .map_err(StartupError::from);
    match (result, shutdown, resource_shutdown) {
        (Err(error), app_cleanup, resource_cleanup) => {
            if let Err(cleanup) = app_cleanup {
                __private::log_cleanup_failure(&cleanup);
            }
            if let Err(cleanup) = resource_cleanup {
                __private::log_cleanup_failure(&cleanup);
            }
            Err(error)
        }
        (Ok(()), Err(error), resource_cleanup) => {
            if let Err(cleanup) = resource_cleanup {
                __private::log_cleanup_failure(&cleanup);
            }
            Err(error)
        }
        (Ok(()), Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(()), Ok(())) => Ok(()),
    }
}

fn print_startup_banner(config: &ScafraConfig, loader: &ConfigLoader, profile: &str) {
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    if !config.startup.show_config {
        println!(
            "\n  {} {}\n",
            paint("Scafra", "36;1", color),
            paint(env!("CARGO_PKG_VERSION"), "37", color)
        );
        return;
    }
    #[cfg(feature = "actuator")]
    let endpoints = config.actuator.enabled_endpoints();
    #[cfg(not(feature = "actuator"))]
    let endpoints: Vec<&str> = Vec::new();
    let endpoints = if endpoints.is_empty() {
        "none".to_owned()
    } else {
        endpoints.join(", ")
    };
    let logo = [
        "  ███████╗███╗   ███╗██████╗ ███████╗██████╗",
        "  ██╔════╝████╗ ████║██╔══██╗██╔════╝██╔══██╗",
        "  █████╗  ██╔████╔██║██████╔╝█████╗  ██████╔╝",
        "  ██╔══╝  ██║╚██╔╝██║██╔══██╗██╔══╝  ██╔══██╗",
        "  ███████╗██║ ╚═╝ ██║██████╔╝███████╗██║  ██║",
        "  ╚══════╝╚═╝     ╚═╝╚═════╝ ╚══════╝╚═╝  ╚═╝",
    ];
    let logo = logo
        .iter()
        .enumerate()
        .map(|(index, line)| paint(line, if index % 2 == 0 { "38;5;208" } else { "33" }, color))
        .collect::<Vec<_>>()
        .join("\n");
    let label = |value: &str| paint(value, "38;5;208;1", color);
    println!(
        "\n{}\n\n  {} {}\n  {}       {}\n  {}       {}\n  {}       {}\n  {}      {}\n  {}     {}\n",
        logo,
        label("Scafra"),
        paint(env!("CARGO_PKG_VERSION"), "37", color),
        label("server"),
        paint(
            &format!("http://{}:{}", config.server.host, config.server.port),
            "37",
            color
        ),
        label("config"),
        paint(
            &loader.configuration_root().display().to_string(),
            "37",
            color
        ),
        label("profile"),
        paint(profile, "35", color),
        label("log level"),
        paint(&config.logging.level.to_string(), "38;5;208", color),
        label("actuator"),
        paint(&endpoints, "37", color),
    );
}

fn paint(value: &str, code: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[{}m{}\x1b[0m", code, value)
    } else {
        value.to_owned()
    }
}

/// Builds HTTP integration settings from the already validated application config.
#[doc(hidden)]
#[allow(clippy::needless_update)]
pub fn router_options(config: &ScafraConfig) -> web::RouterOptions {
    web::RouterOptions {
        #[cfg(feature = "actuator")]
        actuator: config.actuator.clone(),
        #[cfg(feature = "security")]
        security: config.security.clone(),
        request_timeout: config
            .server
            .request_timeout_seconds
            .map(std::time::Duration::from_secs),
        ..Default::default()
    }
}
