use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::BTreeMap,
    env, fs, io,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use crate::{
    errors::ConfigError,
    merge::{
        decode_error, merge_properties, merge_yaml, parse_scalar, parse_yaml, set_path,
        validate_profile, SOURCE_EXTENSIONS,
    },
};

const SCAFRA_ENV_CONTROLS: [&str; 5] = [
    "PROFILE",
    "CONFIG_DIR",
    "CONFIG_FILE",
    "DEV_CHILD",
    "DEV_RELOAD",
];
pub fn load_yaml<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, ConfigError> {
    let path = path.as_ref();
    let display_path = path.display().to_string();
    let source = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: display_path.clone(),
        source,
    })?;
    let value = parse_yaml(&display_path, &source)?;
    validate_framework_input(&value, ConfigCapabilities::default())?;
    validate_resolved_capabilities(&value, ConfigCapabilities::default())?;
    serde_yaml::from_value(value).map_err(|_| ConfigError::Decode {
        location: format!(" from `{display_path}`"),
    })
}

/// A typed configuration value can provide its own validation before startup.
pub trait Config: Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    fn validate(&self) -> Result<(), Self::Error>;

    /// Returns safe, actionable validation details when the error contains no
    /// configuration values. The default keeps arbitrary validator errors
    /// redacted.
    fn validation_details(_error: &Self::Error) -> Option<crate::ValidationError> {
        None
    }
}

/// Marks a typed properties struct with the configuration path it owns.
pub trait ConfigProperties: Config {
    const CONFIG_PREFIX: &'static str;
}

/// Lazily accessed, typed application properties for a service or component.
#[derive(Debug, Clone)]
pub struct Properties<T> {
    loader: ConfigLoader,
    marker: PhantomData<fn() -> T>,
}

impl<T> Default for Properties<T> {
    fn default() -> Self {
        Self {
            loader: ConfigLoader::default(),
            marker: PhantomData,
        }
    }
}

impl<T> Properties<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_loader(loader: ConfigLoader) -> Self {
        Self {
            loader,
            marker: PhantomData,
        }
    }

    pub fn load(&self) -> Result<T, ConfigError>
    where
        T: ConfigProperties + DeserializeOwned + Serialize + Default,
        T::Error: std::fmt::Display,
    {
        self.loader.load_properties()
    }
}

/// Optional capabilities available to one application configuration consumer.
///
/// Cargo features are unified across dependencies. A facade or runner can
/// narrow this mask to its own compiled capabilities even when another direct
/// dependency enables more configuration models. A mask cannot enable code
/// that was not compiled into `scafra-config`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigCapabilities {
    pub security: bool,
    pub jwt: bool,
    pub jwks: bool,
    pub actuator: bool,
    pub scheduler: bool,
    pub bootui: bool,
}

// With no features these values happen to be all false, but other feature
// selections require a non-derived implementation.
#[allow(clippy::derivable_impls)]
impl Default for ConfigCapabilities {
    fn default() -> Self {
        Self {
            security: cfg!(feature = "security"),
            jwt: cfg!(feature = "jwt"),
            jwks: cfg!(feature = "jwks"),
            actuator: cfg!(feature = "actuator"),
            scheduler: cfg!(feature = "scheduler"),
            bootui: cfg!(feature = "bootui"),
        }
    }
}

impl ConfigCapabilities {
    fn intersect(self, other: Self) -> Self {
        let security = self.security && other.security;
        let jwt = security && self.jwt && other.jwt;
        Self {
            security,
            jwt,
            jwks: jwt && self.jwks && other.jwks,
            actuator: self.actuator && other.actuator,
            scheduler: self.scheduler && other.scheduler,
            bootui: self.bootui && other.bootui,
        }
    }
}

/// Loader implementing Scafra's configuration precedence chain:
/// defaults, base YAML/YML/properties files, profile YAML/YML/properties
/// files, sorted environment values, then explicit overrides. Missing files
/// in a selected directory are optional; explicitly selected directories and
/// files must exist. Every other source failure stops loading.
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    source: ConfigSource,
    profile: Option<String>,
    env_prefix: String,
    overrides: BTreeMap<String, String>,
    capabilities: ConfigCapabilities,
}

#[derive(Debug, Clone)]
enum ConfigSource {
    Default,
    ProjectRoot(PathBuf),
    Directory(PathBuf),
    File(PathBuf),
}

#[derive(Debug, Clone)]
enum ResolvedSource {
    Directory(PathBuf),
    File(PathBuf),
}

type MergedConfiguration = (serde_yaml::Value, BTreeMap<String, String>, Option<String>);

impl Default for ConfigLoader {
    fn default() -> Self {
        Self {
            source: ConfigSource::Default,
            profile: None,
            env_prefix: "SCAFRA".to_owned(),
            overrides: BTreeMap::new(),
            capabilities: ConfigCapabilities::default(),
        }
    }
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Narrows the optional capabilities accepted by this loader.
    ///
    /// This is independent of Cargo feature unification and can never enable
    /// a capability unavailable in the compiled configuration crate. Repeated
    /// calls only narrow the existing mask.
    pub fn capabilities(mut self, capabilities: ConfigCapabilities) -> Self {
        self.capabilities = self.capabilities.intersect(capabilities);
        self
    }

    /// Selects an application root. For compatibility, `src/resources/` is
    /// used when that directory exists; otherwise the root itself is used.
    /// The selected application root must exist.
    pub fn root(mut self, root: impl Into<PathBuf>) -> Self {
        self.source = ConfigSource::ProjectRoot(root.into());
        self
    }

    /// Selects an exact configuration directory. The directory must exist.
    /// Relative paths are resolved against the process working directory when
    /// configuration is loaded. Conventional files inside it remain optional.
    pub fn config_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.source = ConfigSource::Directory(directory.into());
        self
    }

    /// Selects one required YAML, YML, or properties file as the base source.
    /// Relative paths are resolved against the process working directory when
    /// configuration is loaded. Profile files, when selected, are read from
    /// the same directory as this file.
    pub fn file(mut self, file: impl Into<PathBuf>) -> Self {
        self.source = ConfigSource::File(file.into());
        self
    }

    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = Some(profile.into());
        self
    }

    pub fn env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }

    pub fn override_value(mut self, path: impl Into<String>, value: impl Into<String>) -> Self {
        self.overrides.insert(path.into(), value.into());
        self
    }

    /// Returns the active profile from an explicit override or the base
    /// application configuration. Missing profile configuration means
    /// `default`.
    pub fn active_profile(&self) -> Result<String, ConfigError> {
        Ok(self
            .selected_profile()?
            .unwrap_or_else(|| "default".to_owned()))
    }

    pub fn load<T>(&self) -> Result<T, ConfigError>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let (merged, origins, _) = self.load_merged::<T>("")?;
        serde_yaml::from_value(merged).map_err(|error| decode_error(error, &origins))
    }

    /// Loads a typed value and returns the profile used to resolve it.
    pub fn load_with_profile<T>(&self) -> Result<(T, String), ConfigError>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let (merged, origins, profile) = self.load_merged::<T>("")?;
        let config =
            serde_yaml::from_value(merged).map_err(|error| decode_error(error, &origins))?;
        Ok((config, profile.unwrap_or_else(|| "default".to_owned())))
    }

    pub fn load_properties<T>(&self) -> Result<T, ConfigError>
    where
        T: ConfigProperties + DeserializeOwned + Serialize + Default,
        T::Error: std::fmt::Display,
    {
        let (merged, origins, _) = self.load_merged::<T>(T::CONFIG_PREFIX)?;
        let value = if T::CONFIG_PREFIX.is_empty() {
            merged
        } else {
            T::CONFIG_PREFIX
                .split('.')
                .try_fold(&merged, |value, segment| match value {
                    serde_yaml::Value::Mapping(map) => {
                        map.get(serde_yaml::Value::String(segment.to_owned()))
                    }
                    _ => None,
                })
                .cloned()
                .unwrap_or_else(|| {
                    serde_yaml::to_value(T::default()).unwrap_or(serde_yaml::Value::Null)
                })
        };
        let config: T =
            serde_yaml::from_value(value).map_err(|error| decode_error(error, &origins))?;
        config
            .validate()
            .map_err(|error| validation_config_error::<T>(&error))?;
        Ok(config)
    }

    fn load_merged<T>(&self, prefix: &str) -> Result<MergedConfiguration, ConfigError>
    where
        T: Serialize + Default,
    {
        let source = self.resolve_source()?;
        let mut schema = serde_yaml::to_value(T::default()).map_err(|_| ConfigError::Serialize)?;
        // Properties own a subtree; both defaults and environment resolution
        // must use the same fully qualified paths as files and overrides.
        if !prefix.is_empty() {
            for segment in prefix.split('.').rev() {
                let mut parent = serde_yaml::Mapping::new();
                parent.insert(serde_yaml::Value::String(segment.to_owned()), schema);
                schema = serde_yaml::Value::Mapping(parent);
            }
        }
        let mut merged = schema.clone();
        let mut origins = BTreeMap::new();
        self.merge_base_sources(&mut merged, &mut origins, &source)?;
        let profile = self.profile_from_base(&merged)?;
        validate_profile(profile.as_deref())?;

        if let Some(profile) = &profile {
            let config_root = source.directory();
            for extension in SOURCE_EXTENSIONS {
                self.merge_file(
                    &mut merged,
                    &mut origins,
                    config_root.join(format!("application-{profile}.{extension}")),
                    false,
                )?;
            }
        }

        self.merge_environment(&mut merged, &schema, &mut origins)?;
        for (path, value) in &self.overrides {
            validate_framework_path(path, self.capabilities)?;
            set_path(
                &mut merged,
                path,
                parse_scalar(value),
                "explicit override",
                None,
                &mut origins,
            )?;
        }

        validate_resolved_capabilities(&merged, self.capabilities)?;
        Ok((merged, origins, profile))
    }

    fn selected_profile(&self) -> Result<Option<String>, ConfigError> {
        let mut base = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
        let mut origins = BTreeMap::new();
        let source = self.resolve_source()?;
        self.merge_base_sources(&mut base, &mut origins, &source)?;
        self.profile_from_base(&base)
    }

    fn profile_from_base(&self, base: &serde_yaml::Value) -> Result<Option<String>, ConfigError> {
        if let Some(profile) = &self.profile {
            return Ok(Some(profile.clone()));
        }
        if let Some(profile) = env::var_os("SCAFRA_PROFILE") {
            return profile
                .into_string()
                .map(Some)
                .map_err(|_| ConfigError::InvalidProfile);
        }

        let active = ["scafra", "profiles", "active"]
            .into_iter()
            .try_fold(base, |value, segment| match value {
                serde_yaml::Value::Mapping(map) => {
                    map.get(serde_yaml::Value::String(segment.to_owned()))
                }
                _ => None,
            })
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_owned);
        Ok(active)
    }

    fn resolve_source(&self) -> Result<ResolvedSource, ConfigError> {
        match &self.source {
            ConfigSource::Default => {
                let directory = env::var_os("SCAFRA_CONFIG_DIR");
                let file = env::var_os("SCAFRA_CONFIG_FILE");
                match (directory, file) {
                    (Some(_), Some(_)) => Err(ConfigError::InvalidValue {
                        path: "SCAFRA_CONFIG_DIR/SCAFRA_CONFIG_FILE".to_owned(),
                        source_kind: "both configuration source selectors cannot be set".to_owned(),
                    }),
                    (Some(directory), None) => {
                        let directory = PathBuf::from(directory);
                        validate_source_directory(&directory, "configuration directory")?;
                        Ok(ResolvedSource::Directory(directory))
                    }
                    (None, Some(file)) => {
                        let file = PathBuf::from(file);
                        validate_source_file(&file)?;
                        Ok(ResolvedSource::File(file))
                    }
                    (None, None) => Ok(ResolvedSource::Directory(resolve_project_root(
                        Path::new("."),
                        false,
                    )?)),
                }
            }
            ConfigSource::ProjectRoot(root) => {
                Ok(ResolvedSource::Directory(resolve_project_root(root, true)?))
            }
            ConfigSource::Directory(directory) => {
                validate_source_directory(directory, "configuration directory")?;
                Ok(ResolvedSource::Directory(directory.clone()))
            }
            ConfigSource::File(file) => {
                validate_source_file(file)?;
                Ok(ResolvedSource::File(file.clone()))
            }
        }
    }

    fn merge_base_sources(
        &self,
        target: &mut serde_yaml::Value,
        origins: &mut BTreeMap<String, String>,
        source: &ResolvedSource,
    ) -> Result<(), ConfigError> {
        match source {
            ResolvedSource::Directory(directory) => {
                for extension in SOURCE_EXTENSIONS {
                    self.merge_file(
                        target,
                        origins,
                        directory.join(format!("application.{extension}")),
                        false,
                    )?;
                }
            }
            ResolvedSource::File(file) => {
                self.merge_file(target, origins, file.clone(), true)?;
            }
        }
        Ok(())
    }

    /// Returns the effective configuration directory, or the parent directory
    /// when a single file is selected. The default convention uses
    /// `src/resources/` when present and otherwise the process working directory.
    pub fn configuration_root(&self) -> PathBuf {
        match self.resolve_source() {
            Ok(source) => source.directory(),
            Err(_) => match &self.source {
                ConfigSource::Default => PathBuf::from("."),
                ConfigSource::ProjectRoot(root)
                | ConfigSource::Directory(root)
                | ConfigSource::File(root) => root.clone(),
            },
        }
    }

    pub fn load_validated<T>(&self) -> Result<T, ConfigError>
    where
        T: Config + DeserializeOwned + Serialize + Default,
        T::Error: std::fmt::Display,
    {
        let config = self.load::<T>()?;
        config
            .validate()
            .map_err(|error| validation_config_error::<T>(&error))?;
        Ok(config)
    }

    /// Loads and validates a typed value and returns the profile used to
    /// resolve it.
    pub fn load_validated_with_profile<T>(&self) -> Result<(T, String), ConfigError>
    where
        T: Config + DeserializeOwned + Serialize + Default,
        T::Error: std::fmt::Display,
    {
        let (config, profile) = self.load_with_profile::<T>()?;
        config
            .validate()
            .map_err(|error| validation_config_error::<T>(&error))?;
        Ok((config, profile))
    }

    fn merge_file(
        &self,
        target: &mut serde_yaml::Value,
        origins: &mut BTreeMap<String, String>,
        path: PathBuf,
        required: bool,
    ) -> Result<(), ConfigError> {
        let display_path = path.display().to_string();
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound && !required => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(missing_source(&path, "configuration file"))
            }
            Err(source) => {
                return Err(ConfigError::Read {
                    path: display_path,
                    source,
                })
            }
        };

        if path.extension().and_then(|extension| extension.to_str()) == Some("properties") {
            let mut input = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
            merge_properties(&mut input, &mut BTreeMap::new(), &display_path, &source)?;
            validate_framework_input(&input, self.capabilities)?;
            merge_properties(target, origins, &display_path, &source)
        } else {
            let value = parse_yaml(&display_path, &source)?;
            validate_framework_input(&value, self.capabilities)?;
            merge_yaml(target, value, "", &display_path, origins)
        }
    }

    fn merge_environment(
        &self,
        target: &mut serde_yaml::Value,
        schema: &serde_yaml::Value,
        origins: &mut BTreeMap<String, String>,
    ) -> Result<(), ConfigError> {
        let prefix = format!("{}_", self.env_prefix.to_ascii_uppercase());
        let mut variables = env::vars_os()
            .filter_map(|(key, value)| Some((key.into_string().ok()?, value)))
            .filter(|(key, _)| key.starts_with(&prefix))
            .collect::<Vec<_>>();
        variables.sort_by(|left, right| left.0.cmp(&right.0));

        for (key, value) in variables {
            let Some(raw_path) = key.strip_prefix(&prefix) else {
                continue;
            };
            if self.env_prefix.eq_ignore_ascii_case("SCAFRA")
                && SCAFRA_ENV_CONTROLS
                    .iter()
                    .any(|control| raw_path.eq_ignore_ascii_case(control))
            {
                continue;
            }
            // Reserved unavailable modules get an actionable diagnostic even
            // though their fields are intentionally absent from the schema.
            let lower = raw_path.to_ascii_lowercase();
            for namespace in ["security", "actuator", "scheduler", "bootui"] {
                if lower == namespace || lower.starts_with(&format!("{namespace}_")) {
                    validate_framework_path(namespace, self.capabilities)?;
                }
            }
            if lower.starts_with("security_jwt_") || lower.starts_with("security__jwt__") {
                validate_framework_path("security.jwt", self.capabilities)?;
            }
            let path = resolve_environment_path(schema, raw_path).map_err(|()| {
                ConfigError::InvalidValue {
                    path: raw_path.to_ascii_lowercase(),
                    source_kind: "environment variable name".to_owned(),
                }
            })?;
            if origins
                .get(&path)
                .is_some_and(|source| source == "environment")
            {
                return Err(ConfigError::InvalidValue {
                    path,
                    source_kind: "environment".to_owned(),
                });
            }
            validate_framework_path(&path, self.capabilities)?;
            let value = value.into_string().map_err(|_| ConfigError::InvalidValue {
                path: path.clone(),
                source_kind: "environment".to_owned(),
            })?;
            set_path(
                target,
                &path,
                parse_scalar(&value),
                "environment",
                None,
                origins,
            )?;
        }
        Ok(())
    }
}

impl ResolvedSource {
    fn directory(&self) -> PathBuf {
        match self {
            Self::Directory(directory) => directory.clone(),
            Self::File(file) => file
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .to_owned(),
        }
    }
}

fn resolve_project_root(root: &Path, required: bool) -> Result<PathBuf, ConfigError> {
    if required {
        validate_source_directory(root, "application root")?;
    }

    let resources = root.join("src").join("resources");
    match fs::metadata(&resources) {
        Ok(metadata) if metadata.is_dir() => Ok(resources),
        Ok(_) => Ok(root.to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(root.to_owned()),
        Err(source) => Err(ConfigError::Read {
            path: resources.display().to_string(),
            source,
        }),
    }
}

fn validate_source_directory(path: &Path, source_kind: &'static str) -> Result<(), ConfigError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid_source_path(path, source_kind, "a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(missing_source(path, source_kind))
        }
        Err(source) => Err(ConfigError::Read {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn validate_source_file(path: &Path) -> Result<(), ConfigError> {
    if !matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yaml" | "yml" | "properties")
    ) {
        return Err(invalid_source_path(
            path,
            "configuration file",
            "a `.yaml`, `.yml`, or `.properties` file",
        ));
    }

    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(invalid_source_path(
            path,
            "configuration file",
            "a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(missing_source(path, "configuration file"))
        }
        Err(source) => Err(ConfigError::Read {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn missing_source(path: &Path, source_kind: &str) -> ConfigError {
    ConfigError::Read {
        path: path.display().to_string(),
        source: io::Error::new(
            io::ErrorKind::NotFound,
            format!("required {source_kind} does not exist"),
        ),
    }
}

fn invalid_source_path(path: &Path, source_kind: &str, expected: &str) -> ConfigError {
    ConfigError::InvalidValue {
        path: path.display().to_string(),
        source_kind: format!("invalid {source_kind} path; expected {expected}"),
    }
}

/// Resolves an environment suffix to a serialized configuration path.
///
/// Double underscores are the explicit path separator. Single-underscore
/// names retain the historical spelling when the serialized schema identifies
/// exactly one path.
fn resolve_environment_path(target: &serde_yaml::Value, raw_path: &str) -> Result<String, ()> {
    if raw_path.is_empty() || !valid_environment_name(raw_path) {
        return Err(());
    }

    if raw_path.contains("__") {
        let parts = raw_path
            .split("__")
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>();
        if parts.iter().any(String::is_empty) || !path_exists(target, &parts, "") {
            return Err(());
        }
        return Ok(parts.join("."));
    }

    let tokens = raw_path.split('_').collect::<Vec<_>>();
    if tokens.iter().any(|token| token.is_empty()) {
        return Err(());
    }
    let mut candidates = std::collections::BTreeSet::new();
    legacy_path_candidates(target, &tokens, 0, "", &mut Vec::new(), &mut candidates);
    if candidates.len() != 1 {
        return Err(());
    }
    candidates.into_iter().next().ok_or(())
}

fn valid_environment_name(name: &str) -> bool {
    name.chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn path_exists(value: &serde_yaml::Value, parts: &[String], parent_path: &str) -> bool {
    let Some((part, remaining)) = parts.split_first() else {
        return !parent_path.is_empty();
    };
    let Some(mapping) = value.as_mapping() else {
        return false;
    };
    let key = serde_yaml::Value::String(part.clone());
    let child_path = append_config_path(parent_path, part);

    if let Some(child) = mapping.get(&key) {
        return remaining.is_empty()
            || path_exists(child, remaining, &child_path)
            || (parent_path == "scheduler.tasks"
                && scheduled_task_schema()
                    .is_some_and(|schema| path_exists(&schema, remaining, &child_path)));
    }
    if is_omitted_secret_field(parent_path, part) {
        return remaining.is_empty();
    }
    if parent_path == "scheduler.tasks" && !remaining.is_empty() {
        return scheduled_task_schema()
            .is_some_and(|schema| path_exists(&schema, remaining, &child_path));
    }
    parent_path != "scheduler.tasks" && mapping.is_empty() && remaining.is_empty()
}

fn legacy_path_candidates(
    value: &serde_yaml::Value,
    tokens: &[&str],
    offset: usize,
    parent_path: &str,
    current_path: &mut Vec<String>,
    candidates: &mut std::collections::BTreeSet<String>,
) {
    if offset == tokens.len() {
        if !current_path.is_empty() {
            candidates.insert(current_path.join("."));
        }
        return;
    }
    let Some(mapping) = value.as_mapping() else {
        return;
    };

    for (key, child) in mapping {
        let Some(key) = key.as_str() else {
            continue;
        };
        let key_tokens = key.split('_').collect::<Vec<_>>();
        let end = offset + key_tokens.len();
        if end > tokens.len()
            || !key_tokens
                .iter()
                .zip(&tokens[offset..end])
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        {
            continue;
        }
        current_path.push(key.to_owned());
        let child_path = append_config_path(parent_path, key);
        legacy_path_candidates(child, tokens, end, &child_path, current_path, candidates);
        current_path.pop();
    }

    for secret_field in omitted_secret_fields(parent_path) {
        let key_tokens = secret_field.split('_').collect::<Vec<_>>();
        let end = offset + key_tokens.len();
        if end == tokens.len()
            && key_tokens
                .iter()
                .zip(&tokens[offset..end])
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        {
            current_path.push((*secret_field).to_owned());
            candidates.insert(current_path.join("."));
            current_path.pop();
        }
    }

    if parent_path == "scheduler.tasks" {
        let Some(task_schema) = scheduled_task_schema() else {
            return;
        };
        for end in (offset + 1)..tokens.len() {
            let task_name = tokens[offset..end].join("_").to_ascii_lowercase();
            current_path.push(task_name.clone());
            legacy_path_candidates(
                &task_schema,
                tokens,
                end,
                &append_config_path(parent_path, &task_name),
                current_path,
                candidates,
            );
            current_path.pop();
        }
    } else if parent_path != "scheduler.tasks" && mapping.is_empty() && offset + 1 == tokens.len() {
        let key = tokens[offset].to_ascii_lowercase();
        current_path.push(key);
        candidates.insert(current_path.join("."));
        current_path.pop();
    }
}

fn omitted_secret_fields(parent_path: &str) -> &'static [&'static str] {
    // These serde-skipped keys are intentionally absent from the serialized
    // default, but remain valid deserialization targets for secret injection.
    match parent_path {
        #[cfg(feature = "security")]
        "security.jwt" => &["secret"],
        #[cfg(feature = "security")]
        "security.basic" => &["password"],
        #[cfg(feature = "actuator")]
        "actuator.security" => &["bearer_token"],
        _ => &[],
    }
}

fn is_omitted_secret_field(parent_path: &str, field: &str) -> bool {
    omitted_secret_fields(parent_path).contains(&field)
}

fn scheduled_task_schema() -> Option<serde_yaml::Value> {
    #[cfg(feature = "scheduler")]
    {
        serde_yaml::to_value(scafra_scheduler::ScheduledTaskConfig::default()).ok()
    }
    #[cfg(not(feature = "scheduler"))]
    {
        None
    }
}

fn append_config_path(parent_path: &str, segment: &str) -> String {
    if parent_path.is_empty() {
        segment.to_owned()
    } else {
        format!("{parent_path}.{segment}")
    }
}

fn validation_config_error<T: Config>(error: &T::Error) -> ConfigError {
    match T::validation_details(error) {
        Some(details) => ConfigError::Validation(format!("{}: {}", details.field, details.message)),
        None => ConfigError::Validation(String::new()),
    }
}

/// Framework-owned namespaces are reserved for every loader, including typed
/// application properties. Validate each selected input before merging so an
/// override cannot erase an unavailable security policy.
fn validate_framework_input(
    input: &serde_yaml::Value,
    capabilities: ConfigCapabilities,
) -> Result<(), ConfigError> {
    let Some(mapping) = input.as_mapping() else {
        return Ok(());
    };
    for (key, value) in mapping {
        let Some(key) = key.as_str() else { continue };
        if framework_namespace(key) {
            validate_framework_value(key, value, capabilities)?;
        }
    }
    Ok(())
}

fn framework_namespace(key: &str) -> bool {
    matches!(
        key,
        "server"
            | "logging"
            | "startup"
            | "security"
            | "actuator"
            | "scheduler"
            | "bootui"
            | "scafra"
    )
}

fn validate_framework_value(
    path: &str,
    value: &serde_yaml::Value,
    capabilities: ConfigCapabilities,
) -> Result<(), ConfigError> {
    validate_framework_path(path, capabilities)?;
    if let Some(mapping) = value.as_mapping() {
        for (key, child) in mapping {
            let Some(key) = key.as_str() else {
                return Err(framework_error(
                    path,
                    "framework configuration keys must be strings",
                ));
            };
            validate_framework_value(&format!("{path}.{key}"), child, capabilities)?;
        }
    }
    Ok(())
}

fn framework_error(path: &str, reason: &str) -> ConfigError {
    ConfigError::InvalidValue {
        path: path.to_owned(),
        source_kind: reason.to_owned(),
    }
}

fn validate_framework_path(
    path: &str,
    capabilities: ConfigCapabilities,
) -> Result<(), ConfigError> {
    let namespace = path.split('.').next().unwrap_or_default();
    if !framework_namespace(namespace) {
        return Ok(());
    }
    let available = (namespace != "security" || capabilities.security)
        && (namespace != "actuator" || capabilities.actuator)
        && (namespace != "scheduler" || capabilities.scheduler)
        && (namespace != "bootui" || capabilities.bootui);
    if !available {
        return Err(framework_error(
            namespace,
            "module is unavailable; enable its Cargo feature or remove its configuration",
        ));
    }
    if (path == "security.jwt" || path.starts_with("security.jwt.")) && !capabilities.jwt {
        return Err(framework_error(
            "security.jwt",
            "JWT is unavailable; enable the jwt Cargo feature",
        ));
    }
    if path == "security.jwt.jwk_set_uri" && !capabilities.jwks {
        return Err(framework_error(
            "security.jwt",
            "remote JWT keys are unavailable; enable the jwks Cargo feature",
        ));
    }
    let mut schema =
        serde_yaml::to_value(crate::ScafraConfig::default()).map_err(|_| ConfigError::Serialize)?;
    // The profile selector is loader-owned, rather than part of ScafraConfig.
    if let Some(mapping) = schema.as_mapping_mut() {
        mapping.insert(
            "scafra".into(),
            serde_yaml::from_str("profiles: {active: null}").map_err(|_| ConfigError::Serialize)?,
        );
    }
    let normalized = path.replace("server.request_timeout_seconds", "server.timeout");
    let parts = normalized.split('.').map(str::to_owned).collect::<Vec<_>>();
    let dynamic_task_entry = cfg!(feature = "scheduler")
        && parts.len() == 3
        && parts[0] == "scheduler"
        && parts[1] == "tasks";
    if dynamic_task_entry || path_exists(&schema, &parts, "") {
        Ok(())
    } else {
        Err(framework_error(
            path,
            "unknown framework configuration field",
        ))
    }
}

// An issuer can be a claim restriction on a locally verified token. Only the
// resolved combination of issuer plus absence of a local secret implies remote
// discovery, so this check runs after all precedence layers have been applied.
fn validate_resolved_capabilities(
    value: &serde_yaml::Value,
    capabilities: ConfigCapabilities,
) -> Result<(), ConfigError> {
    let Some(jwt) = value
        .get("security")
        .and_then(|security| security.get("jwt"))
    else {
        return Ok(());
    };
    let has_issuer = jwt
        .get("issuer_uri")
        .is_some_and(|issuer| !issuer.is_null());
    let has_secret = jwt.get("secret").is_some_and(|secret| !secret.is_null());
    if !capabilities.jwks && has_issuer && !has_secret {
        return Err(framework_error("security.jwt", "issuer discovery is unavailable; enable the jwks Cargo feature or configure a local secret"));
    }
    Ok(())
}
