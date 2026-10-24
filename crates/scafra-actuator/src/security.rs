use crate::ActuatorConfig;
use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use scafra_foundation::http_error::HttpError;

pub(crate) fn protected(headers: &HeaderMap, config: &ActuatorConfig) -> Option<Response> {
    if !config.security.enabled {
        return None;
    }
    let Some(token) = config.security.bearer_token.as_deref() else {
        return Some(error_response(HttpError::Internal));
    };
    if token.trim().is_empty() {
        return Some(error_response(HttpError::Internal));
    }
    if headers.get_all("authorization").iter().count() > 1 {
        return Some(unauthorized());
    }
    let valid = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|candidate| candidate == token);
    (!valid).then(unauthorized)
}

fn unauthorized() -> Response {
    // Actuator security is independently configured and always uses Bearer.
    let error = HttpError::Unauthorized;
    let status = StatusCode::from_u16(error.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, [("www-authenticate", "Bearer")], axum::Json(error)).into_response()
}

fn error_response(error: HttpError) -> Response {
    let status = StatusCode::from_u16(error.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, axum::Json(error)).into_response()
}
