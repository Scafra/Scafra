use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, SystemTime},
};

pub use scafra_foundation as foundation;
#[cfg(any(feature = "web", feature = "scheduler"))]
pub use tokio;
pub use tracing;

/// Runs the embedded debug supervisor used by ordinary `cargo run`.
pub fn run_dev_supervisor(manifest_dir: &str, executable: PathBuf) -> i32 {
    let environment = std::env::var_os("SCAFRA_DEV_POLL_INTERVAL_MS");
    let poll_interval = match dev_poll_interval(environment.as_deref()) {
        Ok(interval) => interval,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    let root = Path::new(manifest_dir);
    let mut snapshot = match dev_snapshot(root) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("Scafra dev reload could not watch the project: {error}");
            return 1;
        }
    };
    let mut child = match spawn_dev_child(&executable) {
        Ok(child) => child,
        Err(error) => {
            eprintln!("Scafra dev reload could not start the application: {error}");
            return 1;
        }
    };
    println!("Scafra dev reload: watching src, Cargo.toml, Cargo.lock, and resources");

    loop {
        thread::sleep(poll_interval);
        let current = match dev_snapshot(root) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                eprintln!("Scafra dev reload could not inspect the project: {error}");
                continue;
            }
        };
        if current == snapshot {
            if child.try_wait().ok().flatten().is_some() {
                return 1;
            }
            continue;
        }
        snapshot = current;
        println!("\nScafra dev reload: watched file changed, rebuilding and restarting...\n");
        stop_dev_child(&mut child);
        let status = Command::new("cargo")
            .arg("build")
            .current_dir(root)
            .status();
        if !matches!(status, Ok(status) if status.success()) {
            eprintln!("Scafra dev reload: build failed; waiting for the next change");
            continue;
        }
        child = match spawn_dev_child(&executable) {
            Ok(child) => child,
            Err(error) => {
                eprintln!("Scafra dev reload could not restart the application: {error}");
                return 1;
            }
        };
    }
}

fn dev_poll_interval(value: Option<&std::ffi::OsStr>) -> Result<Duration, &'static str> {
    let Some(value) = value else {
        return Ok(Duration::from_millis(500));
    };
    let milliseconds = value
        .to_str()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| (100..=60000).contains(value))
        .ok_or(
            "SCAFRA_DEV_POLL_INTERVAL_MS must be an integer between 100 and 60000 milliseconds",
        )?;
    Ok(Duration::from_millis(milliseconds))
}

fn spawn_dev_child(executable: &Path) -> std::io::Result<Child> {
    Command::new(executable)
        .env("SCAFRA_DEV_CHILD", "1")
        .spawn()
}

fn stop_dev_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn dev_snapshot(root: &Path) -> std::io::Result<Vec<(PathBuf, Option<SystemTime>, u64)>> {
    let mut files = Vec::new();
    collect_dev_files(&root.join("src"), &mut files)?;
    for name in ["Cargo.toml", "Cargo.lock"] {
        let path = root.join(name);
        if path.exists() {
            let metadata = fs::metadata(&path)?;
            files.push((path, metadata.modified().ok(), metadata.len()));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn collect_dev_files(
    path: &Path,
    files: &mut Vec<(PathBuf, Option<SystemTime>, u64)>,
) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name == "target" || name == ".git")
        {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect_dev_files(&path, files)?;
        } else if entry.file_type()?.is_file() && is_dev_file(&path) {
            let metadata = fs::metadata(&path)?;
            files.push((path, metadata.modified().ok(), metadata.len()));
        }
    }
    Ok(())
}

fn is_dev_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("rs" | "yml" | "yaml" | "properties")
    )
}

/// Logs a bounded startup failure for the generated application entry point.
///
/// This intentionally does not format the error: startup payloads can
/// contain application-provided messages or secret-bearing configuration.
pub fn log_startup_failure(error: &crate::StartupError) {
    // Configuration can fail before the tracing subscriber is initialized.
    // Only fixed hints are printed: arbitrary error payloads may hold secrets.
    eprintln!("Scafra application_startup_failed ({})", error.kind());
    if let crate::StartupError::Config(config) = error {
        let namespace = match config {
            crate::ConfigError::InvalidValue { path, .. } => [
                "security",
                "actuator",
                "scheduler",
                "bootui",
                "server",
                "logging",
                "startup",
            ]
            .into_iter()
            .find(|name| path == name || path.starts_with(&format!("{name}."))),
            _ => None,
        };
        if let Some(namespace) = namespace {
            eprintln!("Scafra `{namespace}` configuration rejected: a module is unavailable or a field is unknown; check its Cargo feature and configuration keys");
        } else {
            eprintln!("Scafra configuration rejected; check selected sources, Cargo features, value types and required credentials");
        }
    }
    match error {
        crate::StartupError::Graph(scafra_core::GraphError::ProviderFailure(failure)) => {
            log_resource_cleanup_failures(failure.cleanup_failures());
            foundation::error!(
                error_kind = error.kind(),
                provider = failure.provider(),
                operation = failure.operation(),
                cleanup_failure_count = failure.cleanup_failures().len(),
                "application_startup_failed"
            );
        }
        #[cfg(feature = "web")]
        crate::StartupError::Web(scafra_web::WebError::Bind { address, source }) => {
            foundation::error!(
                error_kind = error.kind(),
                web_error_kind = "bind",
                %address,
                source = %source,
                "application_startup_failed"
            );
        }
        #[cfg(feature = "web")]
        crate::StartupError::Web(scafra_web::WebError::DuplicateRoute { method, path, .. }) => {
            foundation::error!(
                error_kind = error.kind(),
                web_error_kind = "duplicate_route",
                %method,
                %path,
                "application_startup_failed"
            );
        }
        #[cfg(feature = "web")]
        crate::StartupError::Web(scafra_web::WebError::Server(source)) => {
            foundation::error!(
                error_kind = error.kind(),
                web_error_kind = "server",
                source = %source,
                "application_startup_failed"
            );
        }
        _ => foundation::error!(error_kind = error.kind(), "application_startup_failed"),
    }
}

/// Logs cleanup failure metadata without formatting application-provided
/// lifecycle messages.
pub fn log_cleanup_failure(error: &crate::StartupError) {
    let failure_count = match error {
        crate::StartupError::Core(error) => {
            let failures = error.lifecycle_cleanup_failures();
            for failure in failures {
                let participant = failure.participant();
                foundation::error!(
                    participant = participant.name(),
                    source = participant.source(),
                    phase = failure.phase().name(),
                    "application_lifecycle_cleanup_failed"
                );
            }
            failures.len()
        }
        crate::StartupError::Graph(error) => {
            let failures = error.cleanup_failures();
            log_resource_cleanup_failures(failures);
            failures.len()
        }
        _ => 0,
    };
    foundation::error!(
        error_kind = "lifecycle_cleanup",
        failure_count,
        "application_cleanup_failed"
    );
}

fn log_resource_cleanup_failures(failures: &[scafra_core::ProviderFailure]) {
    for failure in failures {
        foundation::error!(
            provider = failure.provider(),
            operation = failure.operation(),
            source = failure.source_location(),
            "application_resource_cleanup_failed"
        );
    }
}

#[cfg(test)]
mod dev_poll_tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn development_polling_uses_validated_environment_or_default() {
        assert_eq!(dev_poll_interval(None), Ok(Duration::from_millis(500)));
        for milliseconds in [100, 750, 60000] {
            assert_eq!(
                dev_poll_interval(Some(OsStr::new(&milliseconds.to_string()))),
                Ok(Duration::from_millis(milliseconds))
            );
        }
        for invalid in [
            "0",
            "99",
            "60001",
            "-1",
            "18446744073709551616",
            "secret-value",
        ] {
            let error = dev_poll_interval(Some(OsStr::new(invalid))).unwrap_err();
            assert_eq!(
                error,
                "SCAFRA_DEV_POLL_INTERVAL_MS must be an integer between 100 and 60000 milliseconds"
            );
            assert!(!error.contains("secret-value"));
        }
    }
}
