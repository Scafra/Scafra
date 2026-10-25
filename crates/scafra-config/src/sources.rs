use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::BTreeMap,
    env, fmt, fs, io,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use crate::{
    errors::ConfigError,
    merge::{
        config_path_parts, decode_error, merge_properties, merge_yaml, parse_leaf_value,
        parse_yaml, set_path, validate_profile, SOURCE_EXTENSIONS,
    },
};

const SCAFRA_ENV_CONTROLS: [&str; 6] = [
    "PROFILE",
    "CONFIG_DIR",
    "CONFIG_FILE",
    "DEV_CHILD",
    "DEV_RELOAD",
    "DEV_POLL_INTERVAL_MS",
];
/// Deserializes a domain YAML document without reserving framework namespaces.
/// For framework startup use `ConfigLoader::load_validated::<ScafraConfig>()`.
pub fn load_yaml<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, ConfigError> {
    let path = path.as_ref();
    let display_path = path.display().to_string();
    let source = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: display_path.clone(),
        source,
    })?;
    let value = parse_yaml(&display_path, &source)?;
    serde_yaml::from_value(value).map_err(|_| ConfigError::Decode {
        location: format!(" from `{display_path}`"),
    })
}

/// A typed configuration value can provide its own validation before startup.
pub trait Config: Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Selects Scafra namespace/capability checks for framework-owned settings.
    /// Domain configuration retains its own schema, including `security` and
    /// `server` fields. The standard application configuration opts in.
    const FRAMEWORK_CONFIGURATION: bool = false;

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
#[derive(Clone)]
pub struct ConfigLoader {
    source: ConfigSource,
    profile: Option<String>,
    env_prefix: String,
    overrides: BTreeMap<String, String>,
    capabilities: ConfigCapabilities,
    framework_configuration: bool,
}

impl fmt::Debug for ConfigLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Domain overrides may contain secrets under arbitrary keys. Neither
        // keys nor values are safe to include in diagnostic output.
        f.debug_struct("ConfigLoader")
            .field("source", &self.source)
            .field("profile", &self.profile)
            .field("env_prefix", &self.env_prefix)
            .field("override_count", &self.overrides.len())
            .field("capabilities", &self.capabilities)
            .field("framework_configuration", &self.framework_configuration)
            .finish()
    }
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
            framework_configuration: false,
        }
    }
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables strict validation of Scafra-owned namespaces in every selected
    /// input before merging. Use this for application configuration; generic
    /// domain settings do not reserve any Scafra field names.
    pub fn framework(mut self) -> Self {
        self.framework_configuration = true;
        self
    }

    /// Narrows the optional capabilities accepted by this loader.
    /// Also selects framework namespace validation.
    ///
    /// This is independent of Cargo feature unification and can never enable
    /// a capability unavailable in the compiled configuration crate. Repeated
    /// calls only narrow the existing mask.
    pub fn capabilities(mut self, capabilities: ConfigCapabilities) -> Self {
        self.framework_configuration = true;
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

    /// Loads a domain value using its typed schema. Select `.framework()` for
    /// strict framework namespaces, or use validated `ScafraConfig` loading.
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
        let mut loader = self.clone();
        loader.framework_configuration |= T::FRAMEWORK_CONFIGURATION;
        let (merged, origins, _) = loader.load_merged::<T>(T::CONFIG_PREFIX)?;
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
                let path = config_root.join(format!("application-{profile}.{extension}"));
                // A selected base file can also match the profile convention.
                // Keep its base-source role while reapplying it in the existing
                // extension order; skipping it would change precedence.
                let allow_profile_selector = source.is_explicit_base_file(&path);
                self.merge_file(
                    &mut merged,
                    &mut origins,
                    path,
                    false,
                    allow_profile_selector,
                )?;
            }
        }

        self.merge_environment(&mut merged, &schema, &mut origins)?;
        let mut override_paths = std::collections::BTreeSet::new();
        for (path, value) in &self.overrides {
            let path = if self.framework_configuration {
                canonical_framework_path(path)
            } else {
                path
            };
            if self.framework_configuration && path == "scafra.profiles.active" {
                return Err(framework_error(path, "profile selection precedes value overrides; use ConfigLoader::profile or SCAFRA_PROFILE"));
            }
            if !override_paths.insert(path) {
                return Err(framework_error(
                    path,
                    "duplicate canonical property and compatibility alias",
                ));
            }
            let value = parse_leaf_value(value);
            if self.framework_configuration {
                validate_framework_value(path, &value, self.capabilities)?;
            }
            set_path(
                &mut merged,
                path,
                value,
                "explicit override",
                None,
                &mut origins,
                self.framework_configuration,
            )?;
        }

        if self.framework_configuration {
            validate_resolved_capabilities(&merged, self.capabilities)?;
        }
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
                        true,
                    )?;
                }
            }
            ResolvedSource::File(file) => {
                self.merge_file(target, origins, file.clone(), true, true)?;
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
        let mut loader = self.clone();
        loader.framework_configuration |= T::FRAMEWORK_CONFIGURATION;
        let config = loader.load::<T>()?;
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
        let mut loader = self.clone();
        loader.framework_configuration |= T::FRAMEWORK_CONFIGURATION;
        let (config, profile) = loader.load_with_profile::<T>()?;
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
        allow_profile_selector: bool,
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
            merge_properties(
                &mut input,
                &mut BTreeMap::new(),
                &display_path,
                &source,
                self.framework_configuration,
            )?;
            if self.framework_configuration {
                validate_profile_selector_source(&input, allow_profile_selector)?;
                validate_framework_input(&input, self.capabilities)?;
            }
            if self.framework_configuration {
                normalize_framework_aliases(&mut input)?;
                merge_yaml(target, input, "", &display_path, origins)
            } else {
                merge_properties(target, origins, &display_path, &source, false)
            }
        } else {
            let mut value = parse_yaml(&display_path, &source)?;
            if self.framework_configuration {
                validate_profile_selector_source(&value, allow_profile_selector)?;
                validate_framework_input(&value, self.capabilities)?;
                normalize_framework_aliases(&mut value)?;
            }
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
            if self.framework_configuration
                && matches!(
                    lower.as_str(),
                    "scafra__profiles__active" | "scafra_profiles_active"
                )
            {
                return Err(framework_error("scafra.profiles.active", "profile selection precedes value overrides; use ConfigLoader::profile or SCAFRA_PROFILE"));
            }

            for namespace in ["security", "actuator", "scheduler", "bootui"] {
                if self.framework_configuration
                    && (lower == namespace || lower.starts_with(&format!("{namespace}_")))
                {
                    validate_framework_path(namespace, self.capabilities)?;
                }
            }
            if self.framework_configuration
                && (lower.starts_with("security_jwt_") || lower.starts_with("security__jwt__"))
            {
                validate_framework_path("security.jwt", self.capabilities)?;
            }
            let path = if self.framework_configuration
                && matches!(
                    lower.as_str(),
                    "server_request_timeout_seconds" | "server__request_timeout_seconds"
                ) {
                Ok("server.timeout".to_owned())
            } else {
                resolve_environment_path(schema, raw_path, self.framework_configuration)
            }
            .map_err(|()| ConfigError::InvalidValue {
                path: raw_path.to_ascii_lowercase(),
                source_kind: "environment variable name".to_owned(),
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
            if self.framework_configuration {
                validate_framework_path(&path, self.capabilities)?;
            }
            let value = value.into_string().map_err(|_| ConfigError::InvalidValue {
                path: path.clone(),
                source_kind: "environment".to_owned(),
            })?;
            let value = parse_leaf_value(&value);
            if self.framework_configuration {
                validate_framework_value(&path, &value, self.capabilities)?;
            }
            set_path(
                target,
                &path,
                value,
                "environment",
                None,
                origins,
                self.framework_configuration,
            )?;
        }
        Ok(())
    }
}

impl ResolvedSource {
    fn is_explicit_base_file(&self, candidate: &Path) -> bool {
        let Self::File(selected) = self else {
            return false;
        };
        selected == candidate
            || fs::canonicalize(selected)
                .ok()
                .zip(fs::canonicalize(candidate).ok())
                .is_some_and(|(selected, candidate)| selected == candidate)
    }

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
fn resolve_environment_path(
    target: &serde_yaml::Value,
    raw_path: &str,
    framework_configuration: bool,
) -> Result<String, ()> {
    if raw_path.is_empty() || !valid_environment_name(raw_path) {
        return Err(());
    }

    if raw_path.contains("__") {
        let explicit = raw_path.split("__").collect::<Vec<_>>();
        if framework_configuration
            && explicit.len() == 4
            && explicit[0].eq_ignore_ascii_case("scheduler")
            && explicit[1].eq_ignore_ascii_case("tasks")
        {
            let name = resolve_task_environment_name(explicit[2])?;
            let parts = vec![
                "scheduler".to_owned(),
                "tasks".to_owned(),
                name,
                explicit[3].to_ascii_lowercase(),
            ];
            if !path_exists(target, &parts, "") {
                return Err(());
            }
            return Ok(parts.join("."));
        }
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
    legacy_path_candidates(
        target,
        &tokens,
        0,
        "",
        &mut Vec::new(),
        &mut candidates,
        framework_configuration,
    );
    if candidates.len() != 1 {
        return Err(());
    }
    candidates.into_iter().next().ok_or(())
}

// Dynamic registration names are case-sensitive. Ordinary environment spelling
// is case-insensitive only when it identifies one linked registration. The
// explicit hex spelling also represents dots, double underscores and collisions.
fn resolve_task_environment_name(name: &str) -> Result<String, ()> {
    const HEX_PREFIX: &str = "TASK_HEX_";
    if name
        .get(..HEX_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(HEX_PREFIX))
    {
        let hex = &name[HEX_PREFIX.len()..];
        if hex.is_empty()
            || !hex.len().is_multiple_of(2)
            || hex.len() > 128
            || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(());
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).map_err(|_| ()))
            .collect::<Result<Vec<_>, _>>()?;
        let decoded = String::from_utf8(bytes).map_err(|_| ())?;
        if decoded.is_empty()
            || !decoded
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(());
        }
        return Ok(decoded);
    }
    canonical_task_environment_name(name)
}

fn canonical_task_environment_name(name: &str) -> Result<String, ()> {
    #[cfg(feature = "scheduler")]
    {
        let mut matches = inventory::iter::<scafra_scheduler::ScheduledTaskRegistration>()
            .filter(|registration| registration.name.eq_ignore_ascii_case(name));
        if let Some(registration) = matches.next() {
            if matches.next().is_some() {
                return Err(());
            }
            return Ok(registration.name.to_owned());
        }
    }
    Ok(name.to_ascii_lowercase())
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
    framework_configuration: bool,
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
        legacy_path_candidates(
            child,
            tokens,
            end,
            &child_path,
            current_path,
            candidates,
            framework_configuration,
        );
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
            let requested = tokens[offset..end].join("_");
            let task_name = if framework_configuration {
                let Ok(name) = canonical_task_environment_name(&requested) else {
                    continue;
                };
                name
            } else {
                requested.to_ascii_lowercase()
            };
            current_path.push(task_name.clone());
            legacy_path_candidates(
                &task_schema,
                tokens,
                end,
                &append_config_path(parent_path, &task_name),
                current_path,
                candidates,
                framework_configuration,
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

// Profile selection is complete before profile-specific files are merged.
// Only base inputs may carry the loader-owned selector; a late selector would
// deserialize successfully but could not affect which files were selected.
fn validate_profile_selector_source(
    input: &serde_yaml::Value,
    allow: bool,
) -> Result<(), ConfigError> {
    if !allow
        && input
            .get("scafra")
            .and_then(|value| value.get("profiles"))
            .is_some_and(|value| value.get("active").is_some())
    {
        return Err(framework_error("scafra.profiles.active", "profile-specific files cannot select a profile; use a base application file, ConfigLoader::profile or SCAFRA_PROFILE"));
    }
    Ok(())
}

/// Framework-owned namespaces are reserved when framework validation is selected.
/// Validate each selected input before merging so an
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
            | "shutdown"
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
    if path == "scheduler.tasks" {
        if let Some(tasks) = value.as_mapping() {
            for (name, task) in tasks {
                let Some(name) = name.as_str() else {
                    return Err(framework_error(path, "task names must be strings"));
                };
                let Some(fields) = task.as_mapping() else {
                    return Err(framework_error(
                        path,
                        "each task must be an object with enabled or interval_ms fields",
                    ));
                };
                for (field, value) in fields {
                    let Some(field) = field
                        .as_str()
                        .filter(|field| matches!(*field, "enabled" | "interval_ms"))
                    else {
                        return Err(framework_error(path, "unknown scheduled task field"));
                    };
                    validate_framework_value(
                        &format!("{path}.{name}.{field}"),
                        value,
                        capabilities,
                    )?;
                }
            }
            return Ok(());
        }
    }
    if value.is_sequence() && framework_namespace(path.split('.').next().unwrap_or_default()) {
        // Serde can deserialize some structs from positional sequences. Permit
        // arrays only at schema-declared list leaves, so a parent sequence
        // cannot conceal capability-gated fields from the namespace checks.
        let schema = serde_yaml::to_value(crate::ScafraConfig::default())
            .map_err(|_| ConfigError::Serialize)?;
        let expected = canonical_framework_path(path)
            .split('.')
            .try_fold(&schema, |node, key| node.get(key));
        if !expected.is_some_and(serde_yaml::Value::is_sequence) {
            return Err(framework_error(
                path,
                "a sequence is not supported for this framework property",
            ));
        }
    }
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
    if matches!(
        path,
        "security.jwt.jwk_set_uri"
            | "security.jwt.cache_ttl_seconds"
            | "security.jwt.refresh_cooldown_seconds"
            | "security.jwt.connect_timeout_ms"
            | "security.jwt.request_timeout_ms"
    ) && !capabilities.jwks
    {
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
    let parts = config_path_parts(canonical_framework_path(path), true)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
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

// Normalize before precedence merging. Serde aliases alone would leave both
// spellings beside the serialized default and fail with a duplicate field.
fn canonical_framework_path(path: &str) -> &str {
    match path {
        "server.request_timeout_seconds" => "server.timeout",
        _ => path,
    }
}

fn normalize_framework_aliases(value: &mut serde_yaml::Value) -> Result<(), ConfigError> {
    let Some(server) = value
        .get_mut("server")
        .and_then(serde_yaml::Value::as_mapping_mut)
    else {
        return Ok(());
    };
    let alias = serde_yaml::Value::String("request_timeout_seconds".to_owned());
    let canonical = serde_yaml::Value::String("timeout".to_owned());
    if let Some(value) = server.remove(&alias) {
        if server.contains_key(&canonical) {
            return Err(framework_error(
                "server.timeout",
                "duplicate canonical property and compatibility alias",
            ));
        }
        server.insert(canonical, value);
    }
    Ok(())
}
