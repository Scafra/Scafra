//! Configurable HTTP authentication and route authorization for Scafra.
//!
//! The public API stays at the crate root: [`SecurityConfig`] and related
//! configuration types describe authenticators, [`AuthorizationPolicy`] and
//! [`RouteAuthorizationMetadata`] describe route requirements, and [`layer`]
//! or [`layer_with_route_policies`] applies them to an Axum router.
//!
//! Internally, configuration, policy matching, HTTP middleware, credential
//! authentication, JWT validation, and JWKS retrieval live in separate
//! modules. Security is disabled by default. When enabled, bearer tokens,
//! HTTP Basic credentials, and JWTs validated with HS256 or RSA JWKS are
//! supported when selected. Authentication and authorization failures are denied; malformed
//! or unsupported credentials do not grant access. Shared bearer and Basic
//! credentials authenticate requests but do not provide roles or scopes.
//! Remote RSA keys are cached per router state for five minutes. Unknown `kid`
//! values trigger a bounded refresh, with a 30-second cooldown, and expired
//! keys are never used after a failed refresh. Discovery and JWKS requests have
//! bounded timeouts and response sizes.
//! Credential-bearing configuration types redact tokens, usernames, passwords,
//! HMAC secrets, and issuer/JWKS URLs from their `Debug` output.
//! When application security is enabled, configured bearer tokens must not be
//! empty or whitespace-only and valid values continue to use exact matching.

//!
//! Cargo features are opt-in: the empty default provides Basic and shared
//! bearer authentication; `jwt` adds local HS256 verification (including issuer
//! and audience checks), and `jwks` adds bounded HTTP discovery and remote RSA
//! verification. `jwks` implies `jwt`; `standard` enables both for migration.
//! Call [`SecurityConfig::validate`] before serving. Configuration requesting an
//! unavailable verifier is rejected even when JWT authentication is disabled.
//! Unsupported verifier intent never falls back to another authenticator.
//! Authorization declarations retain their root paths and are shared with
//! HTTP-only code through `scafra-foundation`.

mod authentication;
mod config;
#[cfg(feature = "jwks")]
mod jwks;
#[cfg(not(feature = "jwks"))]
mod jwks {
    // No cache or HTTP client is created for local authentication.
    pub(super) struct JwksCache;
    impl JwksCache {
        pub(super) fn new() -> Self {
            Self
        }
    }
}
#[cfg(feature = "jwt")]
mod jwt;
mod middleware;
mod policy;

pub use config::{BasicAuthConfig, JwtConfig, SecurityConfig};
pub use middleware::{layer, layer_with_route_policies};
pub use policy::{
    AuthorizationMode, AuthorizationPolicy, ControllerAuthorizationRegistration,
    RouteAuthorizationMetadata,
};
