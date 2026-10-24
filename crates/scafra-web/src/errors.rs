use std::{net::SocketAddr, time::Duration};

use axum::{
    http::{header::WWW_AUTHENTICATE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use scafra_foundation::http_error::HttpError;
use scafra_foundation::{capture_error_backtrace, ShutdownPolicy, ShutdownReason};

/// An explicitly selected, header-safe authentication challenge.
///
/// Select the scheme implemented by the application's authenticator. These
/// constructors do not enable authentication or inspect request credentials.
/// Custom schemes can use an application-owned Axum `IntoResponse` type.
#[derive(Debug, Clone)]
pub struct AuthenticationChallenge(HeaderValue);

impl AuthenticationChallenge {
    /// Challenges an application-owned Bearer authenticator.
    pub fn bearer() -> Self {
        Self(HeaderValue::from_static("Bearer"))
    }

    /// Challenges Basic authentication with a quoted, escaped protection realm.
    /// Rejects invalid HTTP header bytes, including CR/LF injection. Configure
    /// the realm once at startup; never derive it from request credentials.
    pub fn basic(realm: &str) -> Result<Self, axum::http::header::InvalidHeaderValue> {
        let realm = realm.replace('\\', "\\\\").replace('"', "\\\"");
        HeaderValue::from_str(&format!("Basic realm=\"{realm}\"")).map(Self)
    }
}

/// Standard application error that can be returned directly from an Scafra
/// route handler. Internal failures log a safe classification, while HTTP
/// responses redact diagnostic context. Use [`Self::public_validation`] only
/// for reviewed client-facing text.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("resource not found")]
    NotFound,

    /// Internal context, redacted from Scafra HTTP responses and generated logs.
    #[error("validation failed: {0}")]
    Validation(String),

    /// An intentionally public explanation; never include untrusted values.
    #[error("{0}")]
    PublicValidation(String),

    #[error("authentication required")]
    /// A genuine authentication failure with its explicitly applicable scheme.
    Unauthorized(AuthenticationChallenge),

    #[error("access denied")]
    Forbidden,

    #[error("internal application error: {0}")]
    Internal(String),
}

impl AppError {
    /// Creates an intentionally public validation explanation (HTTP 400).
    /// Use fixed, reviewed text; raw deserializer errors belong in `Validation`.
    pub fn public_validation(message: impl Into<String>) -> Self {
        Self::PublicValidation(message.into())
    }

    /// Retains private diagnostic context without exposing it in HTTP or logs.
    pub fn internal(error: impl std::error::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error = match self {
            Self::NotFound => HttpError::NotFound,
            Self::Validation(_) => HttpError::Validation,
            Self::PublicValidation(message) => HttpError::PublicValidation(message),
            Self::Unauthorized(challenge) => {
                let mut response = error_response(HttpError::Unauthorized);
                response.headers_mut().insert(WWW_AUTHENTICATE, challenge.0);
                return response;
            }
            Self::Forbidden => HttpError::Forbidden,
            Self::Internal(_) => {
                scafra_foundation::error!(
                    error_kind = "internal_application_error",
                    backtrace = ?capture_error_backtrace(),
                    "request failed"
                );
                HttpError::Internal
            }
        };
        error_response(error)
    }
}

pub(crate) fn error_response(error: HttpError) -> Response {
    // HttpError owns the status mapping; all values are valid HTTP statuses.
    let status = StatusCode::from_u16(error.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, axum::Json(error)).into_response()
}

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("duplicate route {method} {path} declared by {first} and {second}")]
    DuplicateRoute {
        method: String,
        path: String,
        first: &'static str,
        second: &'static str,
    },

    #[error("could not bind Scafra server to {address}: {source}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    #[error("Scafra server failed: {0}")]
    Server(#[source] std::io::Error),
}

#[cfg(feature = "actuator")]
impl From<scafra_actuator::HealthRegistryError> for WebError {
    fn from(error: scafra_actuator::HealthRegistryError) -> Self {
        Self::Server(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    }
}

/// A typed failure from the Scafra server lifecycle boundary.
///
/// The legacy [`WebError`] API intentionally remains unchanged. New code can
/// inspect a graceful-drain timeout without parsing an I/O error, while the
/// compatibility wrappers translate that outcome back to `WebError::Server`.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error(transparent)]
    Web(#[from] WebError),

    #[error(
        "Scafra graceful shutdown exceeded its {:?} deadline after {:?}",
        policy.grace_period(),
        reason
    )]
    GracefulShutdownTimeout {
        reason: ShutdownReason,
        policy: ShutdownPolicy,
    },
}

impl ServerError {
    /// Converts this typed outcome to the source-compatible web error API.
    pub fn into_web_error(self) -> WebError {
        match self {
            Self::Web(error) => error,
            Self::GracefulShutdownTimeout { .. } => WebError::Server(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Scafra graceful shutdown exceeded its configured deadline",
            )),
        }
    }

    /// Returns the shutdown reason that was active when a typed timeout was
    /// reported.
    pub fn timeout_reason(&self) -> Option<ShutdownReason> {
        match self {
            Self::GracefulShutdownTimeout { reason, .. } => Some(*reason),
            Self::Web(_) => None,
        }
    }

    /// Returns the policy that bounded a typed graceful-drain timeout.
    pub fn timeout_policy(&self) -> Option<ShutdownPolicy> {
        match self {
            Self::GracefulShutdownTimeout { policy, .. } => Some(*policy),
            Self::Web(_) => None,
        }
    }

    /// Returns the timeout duration without exposing any error payload.
    pub fn timeout_duration(&self) -> Option<Duration> {
        self.timeout_policy().map(ShutdownPolicy::grace_period)
    }
}
