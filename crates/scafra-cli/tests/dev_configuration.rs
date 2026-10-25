use std::process::Command;

#[test]
fn invalid_polling_controls_fail_before_starting_cargo_and_do_not_echo_values() {
    for arguments in [
        vec!["dev"],
        vec!["dev", "--poll-interval-ms", "cli-secret-value"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_scafra"))
            .args(arguments)
            .env("SCAFRA_DEV_POLL_INTERVAL_MS", "environment-secret-value")
            .output()
            .unwrap();
        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("between 100 and 60000 milliseconds"),
            "{stderr}"
        );
        assert!(!stderr.contains("environment-secret-value"));
        assert!(!stderr.contains("cli-secret-value"));
        assert!(result.stdout.is_empty());
    }
}
