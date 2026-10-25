use scafra_config::{Config, ConfigLoader, ScafraConfig, ServerConfig, ShutdownConfig};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Case {
    directory: PathBuf,
    prefix: String,
}
impl Case {
    fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/cfg111-config-tests")
            .join(format!("{}-{id}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        Self {
            directory,
            prefix: format!("CFG111_{}_{id}", std::process::id()),
        }
    }
    fn loader(&self) -> ConfigLoader {
        ConfigLoader::new()
            .config_dir(&self.directory)
            .env_prefix(&self.prefix)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.directory.join(name), text).unwrap();
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
struct Variable(String);
impl Variable {
    fn new(case: &Case, suffix: &str, value: &str) -> Self {
        let key = format!("{}_{}", case.prefix, suffix);
        assert!(std::env::var_os(&key).is_none());
        std::env::set_var(&key, value);
        Self(key)
    }
}
impl Drop for Variable {
    fn drop(&mut self) {
        std::env::remove_var(&self.0);
    }
}

#[test]
fn defaults_have_finite_valid_resource_budgets() {
    let case = Case::new();
    let config = case.loader().load_validated::<ScafraConfig>().unwrap();
    let server = config.server;
    assert_eq!(server.host, "127.0.0.1");
    assert_eq!(server.port, 8080);
    assert_eq!(server.request_timeout_seconds, Some(30));
    assert_eq!(server.max_request_body_bytes, 1048576);
    assert_eq!(server.header_read_timeout_seconds, 10);
    assert_eq!(server.idle_timeout_seconds, 60);
    assert_eq!(server.max_connections, 1024);
    assert_eq!(server.max_header_bytes, 32768);
    assert!(server.http1_keep_alive);
    assert_eq!(server.http1_max_headers, 100);
    assert_eq!(server.http2_max_concurrent_streams, 200);
    assert_eq!(
        config.shutdown.policy().grace_period(),
        Duration::from_secs(30)
    );
    assert!(config.shutdown.policy().force_after_grace());
}

const YAML: &str = "server:\n  timeout: 45\n  max_request_body_bytes: 2048\n  header_read_timeout_seconds: 3\n  idle_timeout_seconds: 20\n  max_connections: 32\n  max_header_bytes: 16384\n  http1_keep_alive: false\n  http1_max_headers: 50\n  http2_max_concurrent_streams: 80\nshutdown:\n  grace_period_seconds: 5\n  force_after_grace: false\n";
const PROPERTIES: &str = "server.timeout=45\nserver.max_request_body_bytes=2048\nserver.header_read_timeout_seconds=3\nserver.idle_timeout_seconds=20\nserver.max_connections=32\nserver.max_header_bytes=16384\nserver.http1_keep_alive=false\nserver.http1_max_headers=50\nserver.http2_max_concurrent_streams=80\nshutdown.grace_period_seconds=5\nshutdown.force_after_grace=false\n";

#[test]
fn every_supported_file_format_overrides_operational_settings() {
    for (extension, text) in [("yaml", YAML), ("yml", YAML), ("properties", PROPERTIES)] {
        let case = Case::new();
        case.write(&format!("application.{extension}"), text);
        let config = case.loader().load_validated::<ScafraConfig>().unwrap();
        assert_eq!(config.server.request_timeout_seconds, Some(45));
        assert_eq!(config.server.max_request_body_bytes, 2048);
        assert_eq!(config.server.header_read_timeout_seconds, 3);
        assert_eq!(config.server.idle_timeout_seconds, 20);
        assert_eq!(config.server.max_connections, 32);
        assert_eq!(config.server.max_header_bytes, 16384);
        assert!(!config.server.http1_keep_alive);
        assert_eq!(config.server.http1_max_headers, 50);
        assert_eq!(config.server.http2_max_concurrent_streams, 80);
        assert_eq!(
            config.shutdown.policy().grace_period(),
            Duration::from_secs(5)
        );
        assert!(!config.shutdown.policy().force_after_grace());
    }
}

#[test]
fn files_profiles_environment_and_programmatic_precedence_are_preserved() {
    let case = Case::new();
    case.write(
        "application.yaml",
        "server: {max_request_body_bytes: 100}\nshutdown: {grace_period_seconds: 2}\n",
    );
    case.write("application.yml", "server: {max_request_body_bytes: 200}\n");
    case.write(
        "application.properties",
        "server.max_request_body_bytes=300\n",
    );
    assert_eq!(
        case.loader()
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .max_request_body_bytes,
        300
    );
    case.write(
        "application-production.yaml",
        "server: {max_request_body_bytes: 400}\nshutdown: {grace_period_seconds: 4}\n",
    );
    case.write(
        "application-production.yml",
        "server: {max_request_body_bytes: 500}\n",
    );
    case.write(
        "application-production.properties",
        "server.max_request_body_bytes=600\n",
    );
    let loader = case.loader().profile("production");
    assert_eq!(
        loader
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .max_request_body_bytes,
        600
    );
    let _body = Variable::new(&case, "SERVER__MAX_REQUEST_BODY_BYTES", "700");
    let _shutdown = Variable::new(&case, "SHUTDOWN__GRACE_PERIOD_SECONDS", "7");
    let env = loader.load_validated::<ScafraConfig>().unwrap();
    assert_eq!(env.server.max_request_body_bytes, 700);
    assert_eq!(env.shutdown.grace_period_seconds, 7);
    let explicit = loader
        .override_value("server.max_request_body_bytes", "800")
        .override_value("shutdown.grace_period_seconds", "8")
        .load_validated::<ScafraConfig>()
        .unwrap();
    assert_eq!(explicit.server.max_request_body_bytes, 800);
    assert_eq!(explicit.shutdown.grace_period_seconds, 8);
}

#[test]
fn each_new_field_supports_schema_aware_environment_and_programmatic_overrides() {
    for (field, env, value) in [
        ("server.timeout", "SERVER__TIMEOUT", "12"),
        (
            "server.max_request_body_bytes",
            "SERVER__MAX_REQUEST_BODY_BYTES",
            "2048",
        ),
        (
            "server.header_read_timeout_seconds",
            "SERVER__HEADER_READ_TIMEOUT_SECONDS",
            "2",
        ),
        (
            "server.idle_timeout_seconds",
            "SERVER__IDLE_TIMEOUT_SECONDS",
            "5",
        ),
        ("server.max_connections", "SERVER__MAX_CONNECTIONS", "5"),
        (
            "server.max_header_bytes",
            "SERVER__MAX_HEADER_BYTES",
            "8192",
        ),
        (
            "server.http1_keep_alive",
            "SERVER__HTTP1_KEEP_ALIVE",
            "false",
        ),
        (
            "server.http1_max_headers",
            "SERVER__HTTP1_MAX_HEADERS",
            "50",
        ),
        (
            "server.http2_max_concurrent_streams",
            "SERVER__HTTP2_MAX_CONCURRENT_STREAMS",
            "80",
        ),
        (
            "shutdown.grace_period_seconds",
            "SHUTDOWN__GRACE_PERIOD_SECONDS",
            "2",
        ),
        (
            "shutdown.force_after_grace",
            "SHUTDOWN__FORCE_AFTER_GRACE",
            "false",
        ),
    ] {
        let case = Case::new();
        let _var = Variable::new(&case, env, value);
        let environment = case.loader().load_validated::<ScafraConfig>().unwrap();
        let programmatic = Case::new()
            .loader()
            .override_value(field, value)
            .load_validated::<ScafraConfig>()
            .unwrap();
        assert_eq!(
            serde_yaml::to_value(environment).unwrap(),
            serde_yaml::to_value(programmatic).unwrap(),
            "{field}"
        );
    }
}

#[test]
fn omission_uses_default_while_explicit_null_disables_request_timeout() {
    let case = Case::new();
    assert_eq!(
        case.loader()
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .request_timeout_seconds,
        Some(30)
    );
    for (extension, text) in [
        ("yaml", "server: {timeout: null}\n"),
        ("yml", "server: {timeout: null}\n"),
        ("properties", "server.timeout=null\n"),
    ] {
        let path = format!("selected.{extension}");
        case.write(&path, text);
        assert_eq!(
            case.loader()
                .file(case.directory.join(path))
                .load_validated::<ScafraConfig>()
                .unwrap()
                .server
                .request_timeout_seconds,
            None
        );
    }
    let _var = Variable::new(&case, "SERVER__TIMEOUT", "null");
    assert_eq!(
        case.loader()
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .request_timeout_seconds,
        None
    );
}

#[test]
fn compatibility_alias_is_normalized_before_precedence_merging() {
    let case = Case::new();
    case.write(
        "application.yaml",
        "server: {request_timeout_seconds: 11}\n",
    );
    assert_eq!(
        case.loader()
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .request_timeout_seconds,
        Some(11)
    );
    case.write(
        "application-production.properties",
        "server.request_timeout_seconds=12\n",
    );
    let loader = case.loader().profile("production");
    assert_eq!(
        loader
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .request_timeout_seconds,
        Some(12)
    );
    let _var = Variable::new(&case, "SERVER__REQUEST_TIMEOUT_SECONDS", "13");
    assert_eq!(
        loader
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .request_timeout_seconds,
        Some(13)
    );
    assert_eq!(
        loader
            .override_value("server.request_timeout_seconds", "14")
            .load_validated::<ScafraConfig>()
            .unwrap()
            .server
            .request_timeout_seconds,
        Some(14)
    );
}

#[test]
fn conflicting_aliases_and_environment_spellings_fail_without_values() {
    let case = Case::new();
    case.write(
        "application.yaml",
        "server: {timeout: 1, request_timeout_seconds: 2}\n",
    );
    assert!(case.loader().load_validated::<ScafraConfig>().is_err());
    case.write("application.yaml", "{}");
    assert!(case
        .loader()
        .override_value("server.timeout", "1")
        .override_value("server.request_timeout_seconds", "2")
        .load_validated::<ScafraConfig>()
        .is_err());
    let _legacy = Variable::new(&case, "SERVER_MAX_REQUEST_BODY_BYTES", "10");
    let _explicit = Variable::new(&case, "SERVER__MAX_REQUEST_BODY_BYTES", "11");
    assert!(case.loader().load_validated::<ScafraConfig>().is_err());
}

#[test]
fn invalid_and_unknown_operational_settings_fail_early_and_redacted() {
    for (key, invalid) in [
        ("server.http1_max_headers", "0"),
        ("server.http1_max_headers", "1001"),
        ("server.http2_max_concurrent_streams", "0"),
        ("server.http2_max_concurrent_streams", "10001"),
        ("server.http2_max_concurrent_streams", "4294967296"),
        ("server.timeout", "0"),
        ("server.timeout", "86401"),
        ("server.timeout", "-1"),
        ("server.timeout", "18446744073709551616"),
        ("server.port", "65536"),
        ("server.max_request_body_bytes", "0"),
        ("server.max_request_body_bytes", "1073741825"),
        ("server.header_read_timeout_seconds", "0"),
        ("server.header_read_timeout_seconds", "3601"),
        ("server.idle_timeout_seconds", "86401"),
        ("server.max_connections", "0"),
        ("server.max_connections", "1000001"),
        ("server.max_header_bytes", "8191"),
        ("server.max_header_bytes", "1048577"),
        ("shutdown.grace_period_seconds", "0"),
        ("shutdown.grace_period_seconds", "86401"),
        ("shutdown.force_after_grace", "do-not-echo-secret"),
        ("shutdown.typo", "do-not-echo-secret"),
    ] {
        let case = Case::new();
        let error = case
            .loader()
            .override_value(key, invalid)
            .load_validated::<ScafraConfig>()
            .expect_err(key);
        assert!(!format!("{error:?} {error}").contains("do-not-echo-secret"));
    }
    assert!(ServerConfig {
        max_request_body_bytes: 0,
        ..Default::default()
    }
    .validate()
    .is_err());
    assert!(ShutdownConfig {
        grace_period_seconds: 0,
        ..Default::default()
    }
    .validate()
    .is_err());
}

#[cfg(feature = "actuator")]
#[test]
fn actuator_health_timeout_is_validated_after_environment_resolution() {
    let case = Case::new();
    let _var = Variable::new(&case, "ACTUATOR__HEALTH__CHECK_TIMEOUT_MS", "0");
    let error = case.loader().load_validated::<ScafraConfig>().unwrap_err();
    assert!(error
        .validation_details()
        .unwrap()
        .contains("actuator.health.check_timeout_ms"));
}

#[cfg(feature = "security")]
#[test]
fn remote_policy_keys_require_the_callers_jwks_capability_even_at_default_values() {
    for (key, value) in [
        ("cache_ttl_seconds", "300"),
        ("refresh_cooldown_seconds", "30"),
        ("connect_timeout_ms", "2000"),
        ("request_timeout_ms", "5000"),
    ] {
        let case = Case::new();
        let loader = case
            .loader()
            .capabilities(scafra_config::ConfigCapabilities {
                jwks: false,
                ..Default::default()
            });
        let error = loader
            .clone()
            .override_value(format!("security.jwt.{key}"), value)
            .load_validated::<ScafraConfig>()
            .unwrap_err();
        assert!(error.to_string().contains("Cargo feature"));
        let _var = Variable::new(
            &case,
            &format!("SECURITY__JWT__{}", key.to_ascii_uppercase()),
            value,
        );
        assert!(loader.load_validated::<ScafraConfig>().is_err());
    }
}

#[test]
fn module_settings_share_the_file_profile_environment_and_programmatic_contract() {
    let settings = [
        (
            "logging.level",
            "LOGGING__LEVEL",
            "logging: {level: debug}\n",
            "debug",
        ),
        (
            "logging.backtrace",
            "LOGGING__BACKTRACE",
            "logging: {backtrace: errors}\n",
            "errors",
        ),
        (
            "startup.banner",
            "STARTUP__BANNER",
            "startup: {banner: false}\n",
            "false",
        ),
        (
            "startup.show_config",
            "STARTUP__SHOW_CONFIG",
            "startup: {show_config: false}\n",
            "false",
        ),
        #[cfg(feature = "security")]
        (
            "security.hide_unauthorized",
            "SECURITY__HIDE_UNAUTHORIZED",
            "security: {hide_unauthorized: true}\n",
            "true",
        ),
        #[cfg(feature = "jwt")]
        (
            "security.jwt.leeway_seconds",
            "SECURITY__JWT__LEEWAY_SECONDS",
            "security: {jwt: {leeway_seconds: 5}}\n",
            "5",
        ),
        #[cfg(feature = "jwks")]
        (
            "security.jwt.cache_ttl_seconds",
            "SECURITY__JWT__CACHE_TTL_SECONDS",
            "security: {jwt: {cache_ttl_seconds: 120}}\n",
            "120",
        ),
        #[cfg(feature = "jwks")]
        (
            "security.jwt.refresh_cooldown_seconds",
            "SECURITY__JWT__REFRESH_COOLDOWN_SECONDS",
            "security: {jwt: {refresh_cooldown_seconds: 10}}\n",
            "10",
        ),
        #[cfg(feature = "jwks")]
        (
            "security.jwt.connect_timeout_ms",
            "SECURITY__JWT__CONNECT_TIMEOUT_MS",
            "security: {jwt: {connect_timeout_ms: 1000}}\n",
            "1000",
        ),
        #[cfg(feature = "jwks")]
        (
            "security.jwt.request_timeout_ms",
            "SECURITY__JWT__REQUEST_TIMEOUT_MS",
            "security: {jwt: {request_timeout_ms: 3000}}\n",
            "3000",
        ),
        #[cfg(feature = "actuator")]
        (
            "actuator.health.check_timeout_ms",
            "ACTUATOR__HEALTH__CHECK_TIMEOUT_MS",
            "actuator: {health: {check_timeout_ms: 250}}\n",
            "250",
        ),
        #[cfg(feature = "scheduler")]
        (
            "scheduler.enabled",
            "SCHEDULER__ENABLED",
            "scheduler: {enabled: true}\n",
            "true",
        ),
        #[cfg(feature = "bootui")]
        (
            "bootui.port",
            "BOOTUI__PORT",
            "bootui: {port: 8092}\n",
            "8092",
        ),
        #[cfg(feature = "bootui")]
        (
            "bootui.local_only",
            "BOOTUI__LOCAL_ONLY",
            "bootui: {local_only: false}\n",
            "false",
        ),
    ];
    for (path, env, yaml, scalar) in settings {
        let check = |config: ScafraConfig| {
            let document = serde_yaml::to_value(config).unwrap();
            let value = path
                .split('.')
                .fold(&document, |node, key| node.get(key).unwrap());
            assert_eq!(
                value,
                &serde_yaml::from_str::<serde_yaml::Value>(scalar).unwrap(),
                "{path}"
            );
        };
        for extension in ["yaml", "yml", "properties"] {
            let case = Case::new();
            let text = if extension == "properties" {
                format!("{path}={scalar}\n")
            } else {
                yaml.to_owned()
            };
            case.write(&format!("application.{extension}"), &text);
            check(case.loader().load_validated::<ScafraConfig>().unwrap());
            fs::rename(
                case.directory.join(format!("application.{extension}")),
                case.directory
                    .join(format!("application-production.{extension}")),
            )
            .unwrap();
            check(
                case.loader()
                    .profile("production")
                    .load_validated::<ScafraConfig>()
                    .unwrap(),
            );
        }
        let case = Case::new();
        let _var = Variable::new(&case, env, scalar);
        check(case.loader().load_validated::<ScafraConfig>().unwrap());
        drop(_var);
        check(
            case.loader()
                .override_value(path, scalar)
                .load_validated::<ScafraConfig>()
                .unwrap(),
        );
    }
}

#[test]
fn server_ip_literals_validate_without_echoing_invalid_hosts() {
    let case = Case::new();
    for ip in ["127.0.0.1", "0.0.0.0", "::1", "::"] {
        let config = case
            .loader()
            .override_value("server.host", ip)
            .load_validated::<ScafraConfig>()
            .unwrap();
        assert_eq!(config.server.host, ip);
    }
    let error = case
        .loader()
        .override_value("server.host", "invalid-host-secret")
        .load_validated::<ScafraConfig>()
        .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("invalid-host-secret"));
}

#[cfg(feature = "bootui")]
#[test]
fn enabled_bootui_rejects_invalid_network_and_mount_policy_before_startup() {
    let case = Case::new();
    for (field, value) in [
        ("bootui.host", "0.0.0.0"),
        ("bootui.host", "invalid-host-secret"),
        ("bootui.local_only", "false"),
        ("bootui.path", "/"),
        ("bootui.path", "/{capture}"),
        ("bootui.path", "/:legacy"),
        ("bootui.path", "/wild/*"),
        ("bootui.path", "/dashboard?query"),
        ("bootui.path", "/dashboard#fragment"),
    ] {
        let error = case
            .loader()
            .override_value("bootui.enabled", "true")
            .override_value(field, value)
            .load_validated::<ScafraConfig>()
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("invalid-host-secret"));
    }
    #[cfg(feature = "security")]
    {
        let config = case
            .loader()
            .override_value("bootui.enabled", "true")
            .override_value("bootui.host", "::")
            .override_value("bootui.local_only", "false")
            .override_value("security.enabled", "true")
            .override_value("security.bearer_token", "test-only-bootui-token")
            .load_validated::<ScafraConfig>()
            .unwrap();
        assert!(!config.bootui.local_only);
    }
}

#[test]
fn supported_string_lists_work_from_all_sources_and_reject_invalid_elements() {
    let settings: &[(&str, &str, &str, &str)] = &[
        #[cfg(feature = "security")]
        (
            "security.permit_all",
            "SECURITY__PERMIT_ALL",
            "security: {permit_all: [/public, /health]}\n",
            "[/public, /health]",
        ),
        #[cfg(feature = "jwt")]
        (
            "security.jwt.audiences",
            "SECURITY__JWT__AUDIENCES",
            "security: {jwt: {audiences: [api, admin]}}\n",
            "[api, admin]",
        ),
        #[cfg(feature = "jwt")]
        (
            "security.jwt.required_scopes",
            "SECURITY__JWT__REQUIRED_SCOPES",
            "security: {jwt: {required_scopes: [read, write]}}\n",
            "[read, write]",
        ),
        #[cfg(feature = "actuator")]
        (
            "actuator.health.checks",
            "ACTUATOR__HEALTH__CHECKS",
            "actuator: {health: {checks: [database, cache]}}\n",
            "[database, cache]",
        ),
        #[cfg(feature = "actuator")]
        (
            "actuator.endpoints",
            "ACTUATOR__ENDPOINTS",
            "actuator: {endpoints: [health, ready]}\n",
            "[health, ready]",
        ),
    ];
    for &(path, env, yaml, list) in settings {
        let check = |config: ScafraConfig| {
            let document = serde_yaml::to_value(config).unwrap();
            let value = path
                .split('.')
                .fold(&document, |node, key| node.get(key).unwrap());
            assert_eq!(
                value,
                &serde_yaml::from_str::<serde_yaml::Value>(list).unwrap(),
                "{path}"
            );
        };
        for extension in ["yaml", "yml", "properties"] {
            let case = Case::new();
            let text = if extension == "properties" {
                format!("{path}={list}\n")
            } else {
                yaml.to_owned()
            };
            case.write(&format!("application.{extension}"), &text);
            check(case.loader().load_validated::<ScafraConfig>().unwrap());
            fs::rename(
                case.directory.join(format!("application.{extension}")),
                case.directory
                    .join(format!("application-production.{extension}")),
            )
            .unwrap();
            check(
                case.loader()
                    .profile("production")
                    .load_validated::<ScafraConfig>()
                    .unwrap(),
            );
        }
        let case = Case::new();
        let variable = Variable::new(&case, env, list);
        check(case.loader().load_validated::<ScafraConfig>().unwrap());
        drop(variable);
        check(
            case.loader()
                .override_value(path, list)
                .load_validated::<ScafraConfig>()
                .unwrap(),
        );
        let error = case
            .loader()
            .override_value(path, "[{secret: do-not-echo-list-secret}]")
            .load_validated::<ScafraConfig>()
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("do-not-echo-list-secret"));
    }
}

#[test]
fn parent_sequences_cannot_hide_framework_fields_from_schema_checks() {
    let case = Case::new();
    for path in ["server", "logging", "startup", "shutdown"] {
        assert!(case
            .loader()
            .override_value(path, "[]")
            .load_validated::<ScafraConfig>()
            .is_err());
    }
    #[cfg(feature = "security")]
    {
        assert!(case
            .loader()
            .override_value("security", "[false, false, null, {}, [], {}]")
            .load_validated::<ScafraConfig>()
            .is_err());
        let variable = Variable::new(&case, "SECURITY", "[]");
        assert!(case.loader().load_validated::<ScafraConfig>().is_err());
        drop(variable);
        case.write("application.yaml", "security: []\n");
        assert!(case.loader().load_validated::<ScafraConfig>().is_err());
    }
}

#[test]
fn late_profile_selectors_are_rejected_instead_of_accepted_and_ignored() {
    let case = Case::new();
    case.write("application.yaml", "scafra: {profiles: {active: base}}\n");
    case.write("application-base.yaml", "server: {port: 8181}\n");
    case.write("application-production.yaml", "server: {port: 8282}\n");
    let from_base = case.loader().load_validated::<ScafraConfig>().unwrap();
    assert_eq!(from_base.server.port, 8181);
    let explicit = case
        .loader()
        .profile("production")
        .load_validated::<ScafraConfig>()
        .unwrap();
    assert_eq!(explicit.server.port, 8282);
    let error = case
        .loader()
        .override_value("scafra.profiles.active", "do-not-echo-profile-secret")
        .load_validated::<ScafraConfig>()
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("ConfigLoader::profile or SCAFRA_PROFILE"));
    assert!(!format!("{error:?} {error}").contains("do-not-echo-profile-secret"));
    for suffix in ["SCAFRA__PROFILES__ACTIVE", "SCAFRA_PROFILES_ACTIVE"] {
        let variable = Variable::new(&case, suffix, "do-not-echo-profile-secret");
        let error = case.loader().load_validated::<ScafraConfig>().unwrap_err();
        assert!(error.to_string().contains("SCAFRA_PROFILE"));
        assert!(!format!("{error:?} {error}").contains("do-not-echo-profile-secret"));
        drop(variable);
    }
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainProfileSettings {
    scafra: DomainProfileRoot,
}
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainProfileRoot {
    profiles: DomainProfileSelection,
}
#[derive(serde::Deserialize, serde::Serialize)]
struct DomainProfileSelection {
    active: String,
}
impl Default for DomainProfileSelection {
    fn default() -> Self {
        Self {
            active: "default".to_owned(),
        }
    }
}

#[test]
fn generic_domain_loader_retains_late_profile_named_value_overrides() {
    let case = Case::new();
    let config = case
        .loader()
        .override_value("scafra.profiles.active", "custom")
        .load::<DomainProfileSettings>()
        .unwrap();
    assert_eq!(config.scafra.profiles.active, "custom");
    let _variable = Variable::new(&case, "SCAFRA__PROFILES__ACTIVE", "domain");
    let config = case.loader().load::<DomainProfileSettings>().unwrap();
    assert_eq!(config.scafra.profiles.active, "domain");
}

#[test]
fn profile_specific_sources_cannot_change_the_already_selected_profile() {
    for (extension, text) in [
        (
            "yaml",
            "scafra: {profiles: {active: do-not-echo-profile-secret}}\nserver: {port: 8282}\n",
        ),
        (
            "yml",
            "scafra: {profiles: {active: do-not-echo-profile-secret}}\nserver: {port: 8282}\n",
        ),
        (
            "properties",
            "scafra.profiles.active=do-not-echo-profile-secret\nserver.port=8282\n",
        ),
    ] {
        let case = Case::new();
        case.write(
            "application.yaml",
            "scafra: {profiles: {active: production}}\n",
        );
        let selected = format!("application-production.{extension}");
        case.write(&selected, text);
        let error = case.loader().load_validated::<ScafraConfig>().unwrap_err();
        assert!(error
            .to_string()
            .contains("profile-specific files cannot select a profile"));
        assert!(error.to_string().contains("SCAFRA_PROFILE"));
        assert!(!format!("{error:?} {error}").contains("do-not-echo-profile-secret"));
        // The file's role, rather than its filename, decides whether it may
        // select a profile. An explicitly selected file remains a base input.
        let (config, profile) = case
            .loader()
            .file(case.directory.join(&selected))
            .load_validated_with_profile::<ScafraConfig>()
            .unwrap();
        assert_eq!(config.server.port, 8282);
        assert_eq!(profile, "do-not-echo-profile-secret");
        // A domain schema may still own this field as ordinary data.
        let (config, selected_profile) = case
            .loader()
            .load_with_profile::<DomainProfileSettings>()
            .unwrap();
        assert_eq!(config.scafra.profiles.active, "do-not-echo-profile-secret");
        assert_eq!(selected_profile, "production");
    }
}

#[test]
fn explicit_base_matching_profile_filename_keeps_its_role_and_extension_precedence() {
    let extensions = ["yaml", "yml", "properties"];
    for (selected_index, selected_extension) in extensions.iter().enumerate() {
        let case = Case::new();
        for (index, extension) in extensions.iter().enumerate() {
            let port = 8101 + index;
            let selector = index == selected_index;
            let text = if *extension == "properties" {
                format!(
                    "{}server.port={port}\n",
                    if selector {
                        "scafra.profiles.active=production\n"
                    } else {
                        ""
                    }
                )
            } else {
                format!(
                    "{}server: {{port: {port}}}\n",
                    if selector {
                        "scafra: {profiles: {active: production}}\n"
                    } else {
                        ""
                    }
                )
            };
            case.write(&format!("application-production.{extension}"), &text);
        }
        // Parent components are supported; on Unix a distinct symlink spelling
        // also proves that source identity follows the selected file itself.
        fs::create_dir(case.directory.join("nested")).unwrap();
        #[cfg(unix)]
        let filename = {
            let filename = format!("selected.{selected_extension}");
            std::os::unix::fs::symlink(
                format!("application-production.{selected_extension}"),
                case.directory.join(&filename),
            )
            .unwrap();
            filename
        };
        #[cfg(not(unix))]
        let filename = format!("application-production.{selected_extension}");
        let selected = case.directory.join("nested/..").join(filename);
        let loader = case.loader().file(selected);
        let (config, profile) = loader
            .load_validated_with_profile::<ScafraConfig>()
            .unwrap();
        assert_eq!(profile, "production");
        assert_eq!(config.server.port, 8103);
        // Earlier profile extensions must not overwrite the selected base when
        // its own extension is reached again during profile merging.
        for extension in &extensions[selected_index + 1..] {
            fs::remove_file(
                case.directory
                    .join(format!("application-production.{extension}")),
            )
            .unwrap();
        }
        let config = loader.load_validated::<ScafraConfig>().unwrap();
        assert_eq!(usize::from(config.server.port), 8101 + selected_index);
    }
}

#[test]
fn a_different_profile_file_cannot_reselect_an_explicit_bases_profile() {
    let case = Case::new();
    case.write(
        "application-production.yaml",
        "scafra: {profiles: {active: production}}\n",
    );
    case.write(
        "application-production.yml",
        "scafra: {profiles: {active: other}}\n",
    );
    let error = case
        .loader()
        .file(case.directory.join("application-production.yaml"))
        .load_validated::<ScafraConfig>()
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("profile-specific files cannot select a profile"));
}
