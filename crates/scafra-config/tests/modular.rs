use scafra_config::{ConfigLoader, ScafraConfig};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "scafra-modular-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn loader(&self) -> ConfigLoader {
        ConfigLoader::new()
            .config_dir(&self.0)
            .env_prefix("SCAFRA_MODULAR_TEST_UNUSED")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn custom_application_properties_remain_available() {
    let directory = Directory::new();
    fs::write(
        directory.0.join("application.yaml"),
        "mail:\n  transport: smtp\nserver:\n  port: 8181\n",
    )
    .unwrap();
    assert_eq!(
        directory
            .loader()
            .load::<ScafraConfig>()
            .unwrap()
            .server
            .port,
        8181
    );
}

#[test]
fn unknown_framework_fields_are_rejected_without_values() {
    let directory = Directory::new();
    fs::write(
        directory.0.join("application.yaml"),
        "server:\n  prot: do-not-print-this-secret\n",
    )
    .unwrap();
    let error = directory.loader().load::<ScafraConfig>().unwrap_err();
    assert!(error.to_string().contains("server.prot"));
    assert!(!format!("{error:?} {error}").contains("do-not-print-this-secret"));
}

#[test]
fn unavailable_modules_cannot_be_erased_by_overrides() {
    for (name, available) in [
        ("security", cfg!(feature = "security")),
        ("scheduler", cfg!(feature = "scheduler")),
        ("actuator", cfg!(feature = "actuator")),
        ("bootui", cfg!(feature = "bootui")),
    ] {
        if available {
            continue;
        }
        let directory = Directory::new();
        fs::write(
            directory.0.join("application.yaml"),
            format!("{name}: {{}}\n"),
        )
        .unwrap();
        let error = directory
            .loader()
            .override_value(name, "null")
            .load::<ScafraConfig>()
            .unwrap_err();
        assert!(error.to_string().contains(name));
        assert!(error.to_string().contains("Cargo feature"));
    }
}

#[test]
fn unavailable_modules_in_selected_profile_and_properties_fail() {
    for (name, available) in [
        ("security", cfg!(feature = "security")),
        ("scheduler", cfg!(feature = "scheduler")),
        ("actuator", cfg!(feature = "actuator")),
        ("bootui", cfg!(feature = "bootui")),
    ] {
        if available {
            continue;
        }
        let directory = Directory::new();
        fs::write(
            directory.0.join("application-prod.properties"),
            format!("{name}=null\n"),
        )
        .unwrap();
        assert!(directory
            .loader()
            .profile("prod")
            .load::<ScafraConfig>()
            .is_err());
    }
}

#[cfg(all(feature = "security", not(feature = "jwt")))]
#[test]
fn jwt_configuration_requires_jwt_feature() {
    let directory = Directory::new();
    let error = directory
        .loader()
        .override_value("security.jwt.enabled", "true")
        .load::<ScafraConfig>()
        .unwrap_err();
    assert!(error.to_string().contains("jwt Cargo feature"));
}

#[cfg(all(feature = "jwt", not(feature = "jwks")))]
#[test]
fn remote_keys_require_jwks_feature() {
    let directory = Directory::new();
    let error = directory
        .loader()
        .override_value("security.jwt.jwk_set_uri", "https://example.test/keys")
        .load::<ScafraConfig>()
        .unwrap_err();
    assert!(error.to_string().contains("jwks Cargo feature"));
    assert!(!error.to_string().contains("example.test"));
}

#[cfg(feature = "scheduler")]
#[test]
fn dynamic_scheduler_entries_keep_typed_field_validation() {
    let directory = Directory::new();
    let source = directory.0.join("application.yaml");
    fs::write(
        &source,
        "scheduler:\n  tasks:\n    custom_task:\n      enabled: false\n      interval_ms: 1000\n",
    )
    .unwrap();
    let config = directory.loader().load::<ScafraConfig>().unwrap();
    assert_eq!(
        config.scheduler.tasks["custom_task"].interval_ms,
        Some(1000)
    );
    fs::write(
        &source,
        "scheduler:\n  tasks:\n    custom_task:\n      intervl_ms: 1000\n",
    )
    .unwrap();
    assert!(directory.loader().load::<ScafraConfig>().is_err());
}

fn no_capabilities() -> scafra_config::ConfigCapabilities {
    scafra_config::ConfigCapabilities {
        security: false,
        jwt: false,
        jwks: false,
        actuator: false,
        scheduler: false,
        bootui: false,
    }
}

#[test]
fn narrowed_capabilities_reject_selected_module_inputs_even_with_unified_features() {
    for module in ["security", "scheduler", "actuator", "bootui"] {
        let directory = Directory::new();
        let loader = directory.loader().capabilities(no_capabilities());
        // Compiled aggregate defaults remain harmless when no module was selected.
        loader.load::<ScafraConfig>().unwrap();
        assert!(loader
            .clone()
            .override_value(module, "null")
            .load::<ScafraConfig>()
            .is_err());
        fs::write(
            directory.0.join("application.yaml"),
            format!("{module}: {{}}\n"),
        )
        .unwrap();
        assert!(loader
            .clone()
            .override_value(module, "null")
            .load::<ScafraConfig>()
            .is_err());
        fs::remove_file(directory.0.join("application.yaml")).unwrap();
        fs::write(
            directory.0.join("application-prod.properties"),
            format!("{module}=null\n"),
        )
        .unwrap();
        assert!(loader.profile("prod").load::<ScafraConfig>().is_err());
    }
}

#[test]
fn narrowed_capabilities_reject_environment_module_inputs() {
    let directory = Directory::new();
    let prefix = format!("SCAFRA_NARROWED_{}", std::process::id());
    let key = format!("{prefix}_SECURITY__ENABLED");
    let previous = std::env::var_os(&key);
    std::env::set_var(&key, "true");
    let result = directory
        .loader()
        .env_prefix(prefix)
        .capabilities(no_capabilities())
        .load::<ScafraConfig>();
    match previous {
        Some(value) => std::env::set_var(&key, value),
        None => std::env::remove_var(&key),
    }
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("module is unavailable"));
}

#[cfg(feature = "jwt")]
#[test]
fn local_jwt_issuer_and_audience_work_without_remote_keys() {
    let directory = Directory::new();
    let capabilities = scafra_config::ConfigCapabilities {
        jwks: false,
        ..Default::default()
    };
    fs::write(
        directory.0.join("application.yaml"),
        "security:\n  jwt:\n    audiences: [api]\n",
    )
    .unwrap();
    let config = directory
        .loader()
        .capabilities(capabilities)
        .override_value("security.enabled", "true")
        .override_value("security.jwt.enabled", "true")
        .override_value("security.jwt.secret", "local-test-secret")
        .override_value("security.jwt.issuer_uri", "https://issuer.example.test")
        .load_validated::<ScafraConfig>()
        .unwrap();
    assert_eq!(
        config.security.jwt.issuer_uri.as_deref(),
        Some("https://issuer.example.test")
    );
    assert_eq!(config.security.jwt.audiences, ["api"]);
    let error = directory
        .loader()
        .capabilities(capabilities)
        .override_value("security.jwt.issuer_uri", "https://issuer.example.test")
        .load_validated::<ScafraConfig>()
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("issuer discovery is unavailable"));
    assert!(!error.to_string().contains("issuer.example.test"));
}
