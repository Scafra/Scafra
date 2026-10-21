//! Framework-neutral authorization declarations shared by routing and enforcement.

/// Whether a controller or route inherits the application-wide policy,
/// explicitly allows anonymous access, or requires authentication.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AuthorizationMode {
    #[default]
    Inherit,
    Public,
    Protected,
}

/// Authorization requirements declared on a controller or route.
///
/// Every role group must match at least one role. Every listed scope is
/// required. An empty policy with [`AuthorizationMode::Inherit`] uses the
/// application-wide security setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuthorizationPolicy {
    pub mode: AuthorizationMode,
    pub role_groups: &'static [&'static [&'static str]],
    pub required_scopes: &'static [&'static str],
}

impl AuthorizationPolicy {
    /// Inherit the application-wide security setting.
    pub const fn inherit() -> Self {
        Self {
            mode: AuthorizationMode::Inherit,
            role_groups: &[],
            required_scopes: &[],
        }
    }

    /// Explicitly allow anonymous access.
    pub const fn public() -> Self {
        Self {
            mode: AuthorizationMode::Public,
            role_groups: &[],
            required_scopes: &[],
        }
    }

    /// Require authentication.
    pub const fn protected() -> Self {
        Self {
            mode: AuthorizationMode::Protected,
            role_groups: &[],
            required_scopes: &[],
        }
    }

    /// Returns whether this policy explicitly requires authentication.
    pub fn is_protected(self) -> bool {
        self.mode == AuthorizationMode::Protected
    }
}

/// Authorization metadata for one macro-generated controller route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteAuthorizationMetadata {
    pub controller: &'static str,
    pub method: &'static str,
    pub prefix: &'static str,
    pub path: &'static str,
    pub controller_policy: AuthorizationPolicy,
    pub route_policy: AuthorizationPolicy,
}

/// Link-time route policy metadata emitted by the `#[routes]` macro.
pub struct ControllerAuthorizationRegistration {
    pub controller: &'static str,
    pub routes: &'static [RouteAuthorizationMetadata],
}

inventory::collect!(ControllerAuthorizationRegistration);
