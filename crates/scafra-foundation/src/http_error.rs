//! Dependency-neutral HTTP API error contract shared by optional integrations.
//!
//! This module owns classification, status and payload together. It does not
//! depend on an HTTP runtime; integrations only adapt the numeric status and
//! serializable value to their response types.

use serde::{ser::SerializeStruct, Serialize, Serializer};

/// A Scafra-owned HTTP failure with a safe public explanation.
///
/// No variant accepts internal diagnostics. Applications normally return
/// `scafra_web::AppError` or implement Axum's `IntoResponse` for their own errors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HttpError {
    NotFound,
    Validation,
    /// An intentionally public message. Never pass parser errors or secrets.
    PublicValidation(String),
    InvalidJson,
    InvalidJsonData,
    UnsupportedMediaType,
    PayloadTooLarge,
    Unauthorized,
    Forbidden,
    Internal,
    RequestTimeout,
}

impl HttpError {
    /// HTTP status associated with this classification.
    pub const fn status(&self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Validation | Self::PublicValidation(_) | Self::InvalidJson => 400,
            Self::InvalidJsonData => 422,
            Self::UnsupportedMediaType => 415,
            Self::PayloadTooLarge => 413,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::Internal => 500,
            Self::RequestTimeout => 504,
        }
    }

    /// Stable machine-readable classification; clients must not parse messages.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Validation | Self::PublicValidation(_) | Self::InvalidJsonData => {
                "validation_error"
            }
            Self::InvalidJson => "invalid_json",
            Self::UnsupportedMediaType => "unsupported_media_type",
            Self::PayloadTooLarge => "payload_too_large",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Internal => "internal_error",
            Self::RequestTimeout => "request_timeout",
        }
    }

    /// Safe explanation, independent of any internal error or request data.
    pub fn message(&self) -> &str {
        match self {
            Self::NotFound => "resource not found",
            Self::Validation | Self::InvalidJsonData => "request validation failed",
            Self::PublicValidation(message) => message,
            Self::InvalidJson => "invalid JSON request",
            Self::UnsupportedMediaType => "content type must be application/json",
            Self::PayloadTooLarge => "request body too large",
            Self::Unauthorized => "authentication required",
            Self::Forbidden => "access denied",
            Self::Internal => "internal server error",
            Self::RequestTimeout => "request processing timed out",
        }
    }
}

impl Serialize for HttpError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut payload = serializer.serialize_struct("HttpError", 2)?;
        payload.serialize_field("code", self.code())?;
        payload.serialize_field("message", self.message())?;
        payload.end()
    }
}
