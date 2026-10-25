#![cfg(feature = "web")]

use scafra::{ConfigLoader, ScafraConfig, StartupError};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Source(PathBuf);
impl Source {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/cfg-111/loader-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn loader(&self) -> ConfigLoader {
        ConfigLoader::new()
            .config_dir(&self.0)
            .env_prefix("CFG111_STARTUP_UNUSED")
            .override_value("startup.banner", "false")
            .override_value("logging.level", "off")
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn plain_standard_runner_binds_the_programmatic_port() {
    let source = Source::new();
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = occupied.local_addr().unwrap();
    fs::write(source.0.join("application.yaml"), "server: {port: 8081}\n").unwrap();
    let loader = source
        .loader()
        .override_value("server.port", address.port().to_string());
    let error = tokio::time::timeout(Duration::from_secs(2), scafra::run_with_loader(loader))
        .await
        .expect("selected occupied port must fail promptly")
        .unwrap_err();
    assert!(
        matches!(error, StartupError::Web(scafra::WebError::Bind { address: bound, .. }) if bound == address)
    );
}

#[tokio::test]
async fn finalized_builder_and_cleanup_receive_the_authoritative_configuration() {
    use scafra::web::axum::Router;
    use scafra::{AsyncResourceSet, ShutdownPolicy, ShutdownReason};
    struct Resources {
        cancelled: Arc<AtomicBool>,
        finished: Arc<AtomicBool>,
    }
    impl AsyncResourceSet for Resources {
        fn request_shutdown(&self, reason: ShutdownReason) {
            assert_eq!(reason, ShutdownReason::StartupFailure);
            self.cancelled.store(true, Ordering::SeqCst);
        }
        async fn shutdown(
            &self,
            reason: ShutdownReason,
            policy: ShutdownPolicy,
        ) -> Result<(), scafra::core::GraphError> {
            assert_eq!(reason, ShutdownReason::StartupFailure);
            assert!(self.cancelled.load(Ordering::SeqCst));
            assert_eq!(policy, ShutdownPolicy::new(Duration::from_secs(2), false));
            self.finished.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    let source = Source::new();
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = occupied.local_addr().unwrap();
    let loader = source
        .loader()
        .override_value("server.port", address.port().to_string())
        .override_value("server.max_request_body_bytes", "8")
        .override_value("shutdown.grace_period_seconds", "2")
        .override_value("shutdown.force_after_grace", "false");
    let cancelled = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let resources = Resources {
        cancelled: cancelled.clone(),
        finished: finished.clone(),
    };
    let error =
        scafra::run_with_loader_and_resources(loader, move |config: ScafraConfig| async move {
            assert_eq!(config.server.port, address.port());
            assert_eq!(config.server.max_request_body_bytes, 8);
            let router = scafra::web::finish_managed_router(
                Router::new(),
                &[],
                &scafra::router_options(&config),
            )?;
            Ok((router, resources))
        })
        .await
        .unwrap_err();
    assert!(
        matches!(error, StartupError::Web(scafra::WebError::Bind { address: bound, .. }) if bound == address)
    );
    assert!(cancelled.load(Ordering::SeqCst));
    assert!(finished.load(Ordering::SeqCst));
}

#[tokio::test]
async fn invalid_overrides_are_rejected_before_the_application_builder() {
    let source = Source::new();
    let called = AtomicBool::new(false);
    let error = scafra::run_with_loader_and_resources(
        source.loader().override_value("server.port", "0"),
        |_| {
            called.store(true, Ordering::SeqCst);
            async {
                Ok((
                    scafra::web::finish_managed_router(
                        scafra::web::axum::Router::new(),
                        &[],
                        &Default::default(),
                    )?,
                    (),
                ))
            }
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(error, StartupError::Config(_)));
    assert!(!called.load(Ordering::SeqCst));
}

#[cfg(not(feature = "security"))]
#[tokio::test]
async fn a_supplied_loader_cannot_enable_a_capability_absent_from_the_facade() {
    let source = Source::new();
    let loader = source.loader().override_value("security.enabled", "false");
    let error = scafra::run_with_loader(loader).await.unwrap_err();
    assert!(matches!(error, StartupError::Config(_)));
}
