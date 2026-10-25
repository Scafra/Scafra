//! Security configuration types, defaults, and deterministic validation.

use serde::{Deserialize, Serialize};
use std::fmt;

/// HTTP security defaults for a Scafra application.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub hide_unauthorized: bool,
    /// Optional application-wide shared bearer credential. When security is
    /// enabled, configured values must not be empty or whitespace-only. Valid
    /// tokens are matched exactly and are not normalized.
    #[serde(default)]
    pub bearer_token: Option<String>,
    #[serde(default)]
    pub basic: BasicAuthConfig,
    #[serde(default)]
    pub permit_all: Vec<String>,
    #[serde(default)]
    pub jwt: JwtConfig,
}

impl fmt::Debug for SecurityConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecurityConfig")
            .field("enabled", &self.enabled)
            .field("hide_unauthorized", &self.hide_unauthorized)
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("basic", &self.basic)
            .field("permit_all", &self.permit_all)
            .field("jwt", &self.jwt)
            .finish()
    }
}

/// JWT resource-server configuration.
#[derive(Clone, Deserialize, Serialize)]
pub struct JwtConfig {
    #[serde(default)]
    pub enabled: bool,
    /// HMAC secret for local HS256 validation.
    #[serde(default, skip_serializing)]
    pub secret: Option<String>,
    /// OIDC issuer used for discovery and `iss` validation.
    #[serde(default)]
    pub issuer_uri: Option<String>,
    /// Direct JWKS endpoint; avoids issuer discovery when supplied.
    #[serde(default)]
    pub jwk_set_uri: Option<String>,
    #[serde(default)]
    pub audiences: Vec<String>,
    /// Scopes required on every authenticated JWT request unless a route is
    /// explicitly public.
    #[serde(default)]
    pub required_scopes: Vec<String>,
    /// Accepted clock skew for expiry and not-before claims, in seconds (0..=300).
    #[serde(default = "default_leeway_seconds")]
    pub leeway_seconds: u64,
    /// Remote key freshness window, in seconds (1..=86400).
    #[serde(default = "default_cache_ttl_seconds")]
    pub cache_ttl_seconds: u64,
    /// Minimum interval between refresh attempts, in seconds (1..=3600).
    #[serde(default = "default_refresh_cooldown_seconds")]
    pub refresh_cooldown_seconds: u64,
    /// Remote provider connection deadline, in milliseconds (1..=300000).
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    /// Total discovery and key retrieval deadline, in milliseconds (1..=300000).
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
}

fn default_cache_ttl_seconds() -> u64 {
    300
}
fn default_leeway_seconds() -> u64 {
    60
}
fn default_refresh_cooldown_seconds() -> u64 {
    30
}
fn default_connect_timeout_ms() -> u64 {
    2000
}
fn default_request_timeout_ms() -> u64 {
    5000
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            secret: None,
            issuer_uri: None,
            jwk_set_uri: None,
            audiences: Vec::new(),
            required_scopes: Vec::new(),
            leeway_seconds: default_leeway_seconds(),
            cache_ttl_seconds: default_cache_ttl_seconds(),
            refresh_cooldown_seconds: default_refresh_cooldown_seconds(),
            connect_timeout_ms: default_connect_timeout_ms(),
            request_timeout_ms: default_request_timeout_ms(),
        }
    }
}

impl JwtConfig {
    pub(super) fn validate_remote_policy(&self) -> Result<(), &'static str> {
        if !(1..=86400).contains(&self.cache_ttl_seconds) {
            return Err("JWT cache_ttl_seconds must be between 1 and 86400 seconds");
        }
        if !(1..=3600).contains(&self.refresh_cooldown_seconds)
            || self.refresh_cooldown_seconds > self.cache_ttl_seconds
        {
            return Err("JWT refresh_cooldown_seconds must be between 1 and 3600 seconds and not exceed cache_ttl_seconds");
        }
        if !(1..=300000).contains(&self.connect_timeout_ms) {
            return Err("JWT connect_timeout_ms must be between 1 and 300000 milliseconds");
        }
        if !(1..=300000).contains(&self.request_timeout_ms)
            || self.connect_timeout_ms > self.request_timeout_ms
        {
            return Err("JWT request_timeout_ms must be between 1 and 300000 milliseconds and not be less than connect_timeout_ms");
        }
        Ok(())
    }

    fn has_remote_policy_override(&self) -> bool {
        self.cache_ttl_seconds != default_cache_ttl_seconds()
            || self.refresh_cooldown_seconds != default_refresh_cooldown_seconds()
            || self.connect_timeout_ms != default_connect_timeout_ms()
            || self.request_timeout_ms != default_request_timeout_ms()
    }
}

impl fmt::Debug for JwtConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtConfig")
            .field("enabled", &self.enabled)
            .field("secret", &self.secret.as_ref().map(|_| "[REDACTED]"))
            .field(
                "issuer_uri",
                &self.issuer_uri.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "jwk_set_uri",
                &self.jwk_set_uri.as_ref().map(|_| "[REDACTED]"),
            )
            .field("audiences", &self.audiences)
            .field("required_scopes", &self.required_scopes)
            .field("leeway_seconds", &self.leeway_seconds)
            .field("cache_ttl_seconds", &self.cache_ttl_seconds)
            .field("refresh_cooldown_seconds", &self.refresh_cooldown_seconds)
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("request_timeout_ms", &self.request_timeout_ms)
            .finish()
    }
}

/// HTTP Basic credentials.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct BasicAuthConfig {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
}

impl fmt::Debug for BasicAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BasicAuthConfig")
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl SecurityConfig {
    /// Check that enabled authentication has a complete, usable configuration.
    ///
    /// Empty and whitespace-only configured bearer tokens are rejected. A
    /// valid bearer token retains its exact configured value for matching.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.enabled
            && self.bearer_token.is_none()
            && (self.basic.username.is_none() || self.basic.password.is_none())
            && !self.jwt.enabled
        {
            return Err("a bearer token or complete basic credentials are required");
        }
        if self.enabled
            && self
                .bearer_token
                .as_deref()
                .is_some_and(|token| token.trim().is_empty())
        {
            return Err("the bearer token must not be empty or whitespace-only");
        }
        if self.enabled
            && ((self.basic.username.is_some() && self.basic.username.as_deref() == Some(""))
                || (self.basic.password.is_some() && self.basic.password.as_deref() == Some("")))
        {
            return Err("basic credentials must not be empty");
        }
        if self.enabled && (self.basic.username.is_some() != self.basic.password.is_some()) {
            return Err("basic authentication requires both username and password");
        }
        validate_capabilities(&self.jwt)?;
        if self.jwt.leeway_seconds > 300 {
            return Err("JWT leeway_seconds must be between 0 and 300 seconds");
        }
        self.jwt.validate_remote_policy()?;
        #[cfg(feature = "jwt")]
        if self.jwt.enabled {
            validate_jwt_config(&self.jwt)?;
        }
        Ok(())
    }

    /// Whether a path exactly matches one of the configured `permit_all` paths.
    pub fn is_permitted(&self, path: &str) -> bool {
        self.permit_all.iter().any(|permitted| permitted == path)
    }
}

// Check intent even when authentication is disabled: unavailable verification
// must never be accepted as a harmless configuration fallback.
pub(super) fn validate_capabilities(config: &JwtConfig) -> Result<(), &'static str> {
    if !cfg!(feature = "jwt")
        && (config.enabled
            || config.secret.is_some()
            || config.issuer_uri.is_some()
            || config.jwk_set_uri.is_some()
            || !config.audiences.is_empty()
            || !config.required_scopes.is_empty()
            || config.leeway_seconds != default_leeway_seconds()
            || config.has_remote_policy_override())
    {
        return Err("JWT configuration requires the `jwt` feature");
    }
    if !cfg!(feature = "jwks")
        && (config.has_remote_policy_override()
            || config.jwk_set_uri.is_some()
            || (config.issuer_uri.is_some() && config.secret.is_none()))
    {
        return Err("remote JWT configuration requires the `jwks` feature");
    }
    Ok(())
}

#[cfg(feature = "jwt")]
pub(super) fn validate_jwt_config(config: &JwtConfig) -> Result<(), &'static str> {
    if config.secret.is_none() && config.issuer_uri.is_none() && config.jwk_set_uri.is_none() {
        return Err("JWT requires secret, issuer_uri, or jwk_set_uri");
    }
    if config
        .secret
        .as_deref()
        .is_some_and(|secret| secret.trim().is_empty())
    {
        return Err("the JWT secret must not be empty");
    }
    if config.secret.is_some() && config.jwk_set_uri.is_some() {
        return Err("JWT secret cannot be combined with remote JWKS configuration");
    }

    if config
        .issuer_uri
        .as_deref()
        .is_some_and(|issuer| !valid_provider_url(issuer, true))
    {
        return Err("JWT issuer_uri must be a valid HTTP(S) URL");
    }
    if config
        .jwk_set_uri
        .as_deref()
        .is_some_and(|uri| !valid_provider_url(uri, false))
    {
        return Err("JWT jwk_set_uri must be a valid HTTP(S) URL");
    }
    if config
        .audiences
        .iter()
        .any(|audience| audience.trim().is_empty())
    {
        return Err("JWT audiences must not contain empty or whitespace-only values");
    }
    if config
        .required_scopes
        .iter()
        .any(|scope| scope.trim().is_empty())
    {
        return Err("JWT required_scopes must not contain empty or whitespace-only values");
    }
    Ok(())
}

/// Restrict provider URLs to the HTTP schemes used by the existing client.
/// Credentials and fragments are never meaningful for discovery or JWKS URLs.
#[cfg(feature = "jwt")]
pub(super) fn valid_provider_url(value: &str, issuer: bool) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && (!issuer || url.query().is_none())
}
