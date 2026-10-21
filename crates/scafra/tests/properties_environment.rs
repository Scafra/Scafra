use std::{fs, path::PathBuf, process::Command};

use scafra::prelude::*;
use serde::{Deserialize, Serialize};

const PREFIX: &str = "SCFRA_PROPERTIES_78";
const HOST: &str = "smtp.example.test";

#[derive(Debug, Deserialize, Serialize, Config)]
#[config(prefix = "mail")]
struct MailProperties {
    #[config(required)]
    host: String,
    port: u16,
    reply_to: String,
    tls: TlsProperties,
}

impl Default for MailProperties {
    fn default() -> Self {
        Self {
            host: "localhost".to_owned(),
            port: 2525,
            reply_to: String::new(),
            tls: TlsProperties::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct TlsProperties {
    enabled: bool,
}

#[derive(Debug, Default, Deserialize, Serialize, Config)]
struct UnprefixedProperties {
    host: String,
}

#[derive(Debug, Default, Deserialize, Serialize, Config)]
#[config(prefix = "services.mail")]
struct DottedProperties {
    host: String,
}

#[derive(Debug, Default, Deserialize, Serialize, Config)]
#[config(prefix = "mail")]
struct AmbiguousProperties {
    foo_bar: u16,
    foo: FooProperties,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct FooProperties {
    bar: u16,
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        // Each scenario runs in its own process, so its PID isolates fixtures.
        let path =
            std::env::temp_dir().join(format!("scafra-properties-78-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, contents: &str) {
        fs::write(self.0.join(name), contents).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Set variables on child commands only: no process-global environment mutation
// or locks are needed when the test harness runs these cases concurrently.
fn run_case(case: &str, variables: &[(&str, &str)]) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "properties_environment_child", "--nocapture"])
        .env("SCFRA_PROPERTY_CASE", case)
        .env_remove("SCAFRA_PROFILE")
        .env_remove("SCAFRA_CONFIG_DIR")
        .env_remove("SCAFRA_CONFIG_FILE");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with(&format!("{PREFIX}_")) {
            command.env_remove(key);
        }
    }
    for (suffix, value) in variables {
        command.env(format!("{PREFIX}_{suffix}"), value);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{case} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

macro_rules! case {
    ($name:ident, [$($key:literal => $value:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            run_case(stringify!($name), &[$(($key, $value)),*]);
        }
    };
}

case!(environment_only, ["MAIL_HOST" => HOST]);
case!(environment_over_file, ["MAIL_HOST" => HOST]);
case!(environment_over_profile, ["MAIL_HOST" => HOST]);
case!(explicit_override_is_final, ["MAIL_HOST" => HOST]);
case!(empty_prefix, ["HOST" => HOST]);
case!(dotted_prefix, ["SERVICES__MAIL__HOST" => HOST]);
case!(nested_field, ["MAIL__TLS__ENABLED" => "true"]);
case!(underscore_field_legacy, ["MAIL_REPLY_TO" => HOST]);
case!(underscore_field_explicit, ["MAIL__REPLY_TO" => HOST]);
case!(unknown_path, ["MAIL_UNKNOWN_FIELD" => "secret-sentinel"]);
case!(ambiguous_path, ["MAIL_FOO_BAR" => "7"]);
case!(explicit_ambiguous_path, ["MAIL__FOO_BAR" => "7", "MAIL__FOO__BAR" => "8"]);
case!(duplicate_paths, ["MAIL_HOST" => HOST, "MAIL__HOST" => "other-secret-sentinel"]);
case!(invalid_typed_value, ["MAIL_PORT" => "invalid-secret-sentinel"]);
case!(required_validation, ["MAIL_HOST" => "''"]);
case!(ordinary_load, ["HOST" => HOST]);
case!(file_only, []);
case!(profile_only, []);
case!(defaults_only, []);
case!(explicit_file, ["MAIL_HOST" => HOST]);
case!(missing_explicit_sources, ["MAIL_HOST" => HOST]);

#[test]
fn properties_environment_child() {
    let Ok(case) = std::env::var("SCFRA_PROPERTY_CASE") else {
        return;
    };
    let root = TempDir::new();
    let mut loader = ConfigLoader::new().config_dir(&root.0).env_prefix(PREFIX);
    match case.as_str() {
        "environment_over_file" | "file_only" => {
            root.write("application.yaml", "mail:\n  host: file.example.test\n");
        }
        "environment_over_profile" | "explicit_override_is_final" | "profile_only" => {
            root.write(
                "application.yaml",
                "scafra:\n  profiles:\n    active: test\nmail:\n  host: base.example.test\n",
            );
            root.write(
                "application-test.properties",
                "mail.host=profile.example.test\n",
            );
            if case == "explicit_override_is_final" {
                loader = loader.override_value("mail.host", "override.example.test");
            }
        }
        "explicit_file" => {
            root.write("selected.yml", "mail:\n  host: selected.example.test\n");
            root.write("application.yaml", "mail: invalid-ignored-file\n");
            root.write("application-test.properties", "mail.port=465\n");
            loader = loader.file(root.0.join("selected.yml")).profile("test");
        }
        "unknown_path" => {
            // File contents must not broaden the typed environment schema.
            root.write("application.yaml", "mail:\n  unknown_field: file-value\n");
        }
        _ => {}
    }

    match case.as_str() {
        "empty_prefix" => {
            let value = Properties::<UnprefixedProperties>::with_loader(loader)
                .load()
                .unwrap();
            assert_eq!(value.host, HOST);
        }
        "dotted_prefix" => {
            let value = Properties::<DottedProperties>::with_loader(loader)
                .load()
                .unwrap();
            assert_eq!(value.host, HOST);
        }
        "ordinary_load" => {
            assert_eq!(loader.load::<UnprefixedProperties>().unwrap().host, HOST);
        }
        "ambiguous_path" | "explicit_ambiguous_path" => {
            let result = Properties::<AmbiguousProperties>::with_loader(loader).load();
            if case == "ambiguous_path" {
                assert!(
                    matches!(result, Err(ConfigError::InvalidValue { path, source_kind })
                    if path == "mail_foo_bar" && source_kind == "environment variable name")
                );
            } else {
                let value = result.unwrap();
                assert_eq!(value.foo_bar, 7);
                assert_eq!(value.foo.bar, 8);
            }
        }
        "missing_explicit_sources" => {
            for loader in [
                loader.clone().file(root.0.join("missing.yaml")),
                loader.config_dir(root.0.join("missing")),
            ] {
                assert!(
                    matches!(Properties::<MailProperties>::with_loader(loader).load(),
                    Err(ConfigError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound)
                );
            }
        }
        _ => {
            let result = Properties::<MailProperties>::with_loader(loader).load();
            match case.as_str() {
                "unknown_path" => assert!(matches!(result,
                    Err(ConfigError::InvalidValue { path, source_kind })
                    if path == "mail_unknown_field" && source_kind == "environment variable name")),
                "duplicate_paths" => assert!(matches!(result,
                    Err(ConfigError::InvalidValue { path, source_kind })
                    if path == "mail.host" && source_kind == "environment")),
                "invalid_typed_value" => {
                    let error = result.unwrap_err();
                    assert!(matches!(error, ConfigError::Decode { .. }));
                    assert!(error.to_string().contains("mail.port"));
                    assert!(error.to_string().contains("environment"));
                    assert!(!error.to_string().contains("invalid-secret-sentinel"));
                    assert!(!format!("{error:?}").contains("invalid-secret-sentinel"));
                }
                "required_validation" => {
                    let error = result.unwrap_err();
                    assert!(matches!(error, ConfigError::Validation(_)));
                    assert!(error.validation_details().unwrap().contains("host"));
                }
                _ => {
                    let value = result.unwrap();
                    let expected_host = match case.as_str() {
                        "environment_only"
                        | "environment_over_file"
                        | "environment_over_profile"
                        | "explicit_file" => HOST,
                        "explicit_override_is_final" => "override.example.test",
                        "file_only" => "file.example.test",
                        "profile_only" => "profile.example.test",
                        _ => "localhost",
                    };
                    assert_eq!(value.host, expected_host);
                    assert_eq!(value.port, if case == "explicit_file" { 465 } else { 2525 });
                    assert_eq!(value.tls.enabled, case == "nested_field");
                    assert_eq!(
                        value.reply_to,
                        if case.starts_with("underscore_field") {
                            HOST
                        } else {
                            ""
                        }
                    );
                }
            }
        }
    }
}
