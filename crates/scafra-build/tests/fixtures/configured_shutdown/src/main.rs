#[scafra::main]
async fn main() {}

#[cfg(test)]
mod tests {
    use scafra::{AsyncResourceSet, ConfigLoader, ScafraConfig, ShutdownReason};
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn resolved_policy_reaches_normal_shutdown_and_partial_startup_cleanup() {
        let config = ConfigLoader::new()
            .env_prefix("CFG111_RESOURCE_UNUSED")
            .override_value("shutdown.grace_period_seconds", "3")
            .override_value("shutdown.force_after_grace", "false")
            .load_validated::<ScafraConfig>()
            .unwrap();
        let policy = config.shutdown.policy();
        let graph = super::__scafra_graph::compose_async_with_policy(
            super::__scafra_graph::Overrides::default(),
            policy,
        )
        .await
        .unwrap();
        graph
            .into_resources()
            .shutdown(ShutdownReason::ApplicationRequest, policy)
            .await
            .unwrap();
        assert_eq!(
            *crate::providers::POLICIES.lock().unwrap(),
            vec![
                (ShutdownReason::ApplicationRequest, policy),
                (ShutdownReason::ApplicationRequest, policy),
            ]
        );
        crate::providers::POLICIES.lock().unwrap().clear();
        crate::providers::FAIL_SECOND.store(true, Ordering::SeqCst);
        assert!(super::__scafra_graph::compose_async_with_policy(
            super::__scafra_graph::Overrides::default(),
            policy,
        )
        .await
        .is_err());
        assert_eq!(
            *crate::providers::POLICIES.lock().unwrap(),
            vec![(ShutdownReason::StartupFailure, policy),]
        );
    }
}
