#![cfg(all(feature = "web", feature = "scheduler"))]

use scafra::{AsyncResourceSet, ShutdownPolicy, ShutdownReason};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

static STARTED: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);
static FINISHED: AtomicBool = AtomicBool::new(false);

fn wait_for_resource_cancellation() {
    STARTED.store(true, Ordering::SeqCst);
    while !STOP.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(1));
    }
    FINISHED.store(true, Ordering::SeqCst);
}
scafra::register_scheduled_task!("startup_dependency", 1, wait_for_resource_cancellation);

struct Resources;
impl AsyncResourceSet for Resources {
    fn request_shutdown(&self, reason: ShutdownReason) {
        assert_eq!(reason, ShutdownReason::StartupFailure);
        CANCELLED.store(true, Ordering::SeqCst);
        STOP.store(true, Ordering::SeqCst);
    }
    async fn shutdown(
        &self,
        reason: ShutdownReason,
        _: ShutdownPolicy,
    ) -> Result<(), scafra::core::GraphError> {
        assert_eq!(reason, ShutdownReason::StartupFailure);
        assert!(CANCELLED.load(Ordering::SeqCst));
        assert!(FINISHED.load(Ordering::SeqCst));
        Ok(())
    }
}
struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl Environment {
    fn set(values: &[(&'static str, String)]) -> Self {
        let old = values
            .iter()
            .map(|(name, value)| {
                let old = std::env::var_os(name);
                std::env::set_var(name, value);
                (*name, old)
            })
            .collect();
        Self(old)
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        STOP.store(true, Ordering::SeqCst);
        for (name, old) in &self.0 {
            match old {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bind_failure_cancels_resource_dependencies_before_joining_active_jobs() {
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = occupied.local_addr().unwrap();
    let _environment = Environment::set(&[
        ("SCAFRA_SERVER_PORT", address.port().to_string()),
        ("SCAFRA_SERVER_HOST", "127.0.0.1".into()),
        ("SCAFRA_SCHEDULER_ENABLED", "true".into()),
        ("SCAFRA_STARTUP_BANNER", "false".into()),
        ("SCAFRA_LOGGING_LEVEL", "off".into()),
    ]);
    // Raw-router middleware finalization occurs after scheduler startup. This
    // large, valid composition lets the 1 ms job enter before the bind fails.
    let mut router = scafra::web::axum::Router::new();
    for id in 0..20_000 {
        router = router.route(
            &format!("/cleanup/{id}"),
            scafra::web::axum::routing::get(|| async { "ok" }),
        );
    }
    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        scafra::run_with_resources(move |_| async move { Ok((router, Resources)) }),
    )
    .await;
    // Release the test callback even if the regression recurs, so Tokio's
    // blocking-pool teardown cannot hang the test process after an assertion.
    STOP.store(true, Ordering::SeqCst);
    assert!(
        STARTED.load(Ordering::SeqCst),
        "the dependent job must be active to exercise the cycle"
    );
    let error = outcome
        .expect("cleanup must not deadlock on its own cancellation request")
        .unwrap_err();
    assert!(
        matches!(error, scafra::StartupError::Web(scafra::WebError::Bind { address: bound, .. }) if bound == address)
    );
    assert!(CANCELLED.load(Ordering::SeqCst));
    assert!(FINISHED.load(Ordering::SeqCst));
}
