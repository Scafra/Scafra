use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TARGET: AtomicU64 = AtomicU64::new(0);

struct TemporaryTarget(PathBuf);

impl TemporaryTarget {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "scafra-build-graph-compile-pass-{}-{}",
            std::process::id(),
            NEXT_TARGET.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("temporary cargo target should be creatable");
        Self(path)
    }
}

impl Drop for TemporaryTarget {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn provider_contract_composes_through_the_public_application_graph() {
    let target = TemporaryTarget::new();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/provider_contract/Cargo.toml");
    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(manifest)
        .env("CARGO_TARGET_DIR", &target.0)
        .output()
        .expect("provider contract fixture should run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "provider contract fixture failed:\n{stderr}"
    );
}

#[test]
fn configured_shutdown_policy_reaches_generated_resource_cleanup() {
    let target = TemporaryTarget::new();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/configured_shutdown/Cargo.toml");
    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(manifest)
        .env("CARGO_TARGET_DIR", &target.0)
        .output()
        .expect("configured shutdown fixture should run");
    assert!(
        output.status.success(),
        "configured shutdown fixture failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
