use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use scafra_config::{ConfigError, ConfigLoader, ScafraConfig};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "scafra-config-resolution-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary test directory should be created");
        Self(path)
    }

    fn new_in(parent: &Path) -> Self {
        let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            "scafra-config-resolution-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary test directory should be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exact_absolute_and_relative_directories_load_from_the_selected_location() {
    let cwd = std::env::current_dir().unwrap();
    let relative_temp = TempDir::new_in(&cwd);
    let temp = TempDir::new();
    let config_dir = temp.path().join("config");
    fs::create_dir(&config_dir).unwrap();
    fs::write(
        config_dir.join("application.yaml"),
        "server:\n  port: 8181\n",
    )
    .unwrap();

    let absolute = ConfigLoader::new()
        .config_dir(&config_dir)
        .load::<ScafraConfig>()
        .unwrap();
    assert_eq!(absolute.server.port, 8181);

    let relative_dir = relative_temp.path().join("config");
    fs::create_dir(&relative_dir).unwrap();
    fs::write(
        relative_dir.join("application.yaml"),
        "server:\n  port: 8187\n",
    )
    .unwrap();
    let relative = ConfigLoader::new()
        .config_dir(relative_dir.strip_prefix(&cwd).unwrap())
        .load::<ScafraConfig>()
        .unwrap();
    assert_eq!(relative.server.port, 8187);
}

#[test]
fn explicit_file_is_required_and_does_not_fall_back_to_directory_conventions() {
    let temp = TempDir::new();
    fs::write(
        temp.path().join("application.yaml"),
        "server:\n  port: 8182\n",
    )
    .unwrap();
    let selected = temp.path().join("selected.properties");
    fs::write(&selected, "server.port=8183\n").unwrap();

    let config = ConfigLoader::new()
        .file(&selected)
        .load::<ScafraConfig>()
        .unwrap();
    assert_eq!(config.server.port, 8183);

    let missing = temp.path().join("missing.yaml");
    let error = ConfigLoader::new()
        .file(&missing)
        .load::<ScafraConfig>()
        .expect_err("an explicitly selected file must exist");
    assert!(matches!(
        error,
        ConfigError::Read { ref source, .. } if source.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(error.to_string().contains("missing.yaml"));
}

#[test]
fn explicit_source_paths_are_typed_and_empty_conventional_directories_are_optional() {
    let temp = TempDir::new();
    let missing_dir = temp.path().join("missing");
    let error = ConfigLoader::new()
        .config_dir(&missing_dir)
        .load::<ScafraConfig>()
        .expect_err("an explicitly selected directory must exist");
    assert!(matches!(
        error,
        ConfigError::Read { source, .. } if source.kind() == std::io::ErrorKind::NotFound
    ));

    let root_error = ConfigLoader::new()
        .root(&missing_dir)
        .load::<ScafraConfig>()
        .expect_err("a selected application root must exist");
    assert!(matches!(
        root_error,
        ConfigError::Read { source, .. } if source.kind() == std::io::ErrorKind::NotFound
    ));

    let file_instead_of_dir = temp.path().join("not-a-directory");
    fs::write(&file_instead_of_dir, "data").unwrap();
    let error = ConfigLoader::new()
        .config_dir(&file_instead_of_dir)
        .load::<ScafraConfig>()
        .expect_err("a file cannot be selected as a configuration directory");
    assert!(matches!(
        error,
        ConfigError::InvalidValue { ref source_kind, .. }
            if source_kind.contains("expected a directory")
    ));

    let dir_instead_of_file = temp.path().join("directory.yaml");
    fs::create_dir(&dir_instead_of_file).unwrap();
    let error = ConfigLoader::new()
        .file(&dir_instead_of_file)
        .load::<ScafraConfig>()
        .expect_err("a directory cannot be selected as a configuration file");
    assert!(matches!(
        error,
        ConfigError::InvalidValue { ref source_kind, .. }
            if source_kind.contains("expected a regular file")
    ));

    let empty = temp.path().join("empty-config");
    fs::create_dir(&empty).unwrap();
    let config = ConfigLoader::new()
        .config_dir(&empty)
        .load::<ScafraConfig>()
        .expect("conventional files in an existing directory remain optional");
    assert_eq!(config.server.port, 8080);
}

#[test]
fn invalid_or_unreadable_explicit_files_fail_without_fallback_or_value_leaks() {
    let temp = TempDir::new();
    fs::write(
        temp.path().join("application.yaml"),
        "server:\n  port: 8184\n",
    )
    .unwrap();

    let invalid = temp.path().join("invalid.yaml");
    fs::write(&invalid, "server: [this is not a valid server value\n").unwrap();
    let error = ConfigLoader::new()
        .file(&invalid)
        .load::<ScafraConfig>()
        .expect_err("invalid explicit file contents must stop loading");
    assert!(matches!(error, ConfigError::Parse { .. }));
    assert!(error.to_string().contains("invalid.yaml"));

    let unreadable = temp.path().join("unreadable.properties");
    fs::write(&unreadable, b"server.port=secret\xff\n").unwrap();
    let error = ConfigLoader::new()
        .file(&unreadable)
        .load::<ScafraConfig>()
        .expect_err("invalid text encoding must stop loading");
    assert!(matches!(error, ConfigError::Read { .. }));
    assert!(!error.to_string().contains("secret"));
}

#[test]
fn an_explicit_file_uses_the_selected_profile_and_returns_its_effective_name() {
    let temp = TempDir::new();
    let base = temp.path().join("base.yaml");
    fs::write(
        &base,
        "scafra:\n  profiles:\n    active: local\nserver:\n  port: 8185\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("application-local.yaml"),
        "server:\n  port: 8186\n",
    )
    .unwrap();

    let (config, profile) = ConfigLoader::new()
        .file(&base)
        .load_validated_with_profile::<ScafraConfig>()
        .unwrap();
    assert_eq!(profile, "local");
    assert_eq!(config.server.port, 8186);
}
