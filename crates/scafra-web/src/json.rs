use axum::extract::{
    rejection::{BytesRejection, FailedToBufferBody, JsonRejection},
    FromRequest, Json,
};
use axum::response::Response;
use scafra_foundation::http_error::HttpError;
use serde::de::DeserializeOwned;

use crate::errors::error_response;

/// A typed JSON request body with Scafra's stable rejection responses.
///
/// This extractor delegates deserialization and body buffering to Axum. It is
/// request-only; use Axum's [`Json`] or an application-owned response type for
/// response serialization.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonBody<T>(pub T);

impl<T> JsonBody<T> {
    /// Returns the deserialized request body.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for JsonBody<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T, S> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(
        request: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(json_rejection_response)
    }
}

fn json_rejection_response(rejection: JsonRejection) -> Response {
    let error = match rejection {
        JsonRejection::JsonSyntaxError(_) => HttpError::InvalidJson,
        JsonRejection::JsonDataError(_) => HttpError::InvalidJsonData,
        JsonRejection::MissingJsonContentType(_) => HttpError::UnsupportedMediaType,
        JsonRejection::BytesRejection(BytesRejection::FailedToBufferBody(
            FailedToBufferBody::LengthLimitError(_),
        )) => HttpError::PayloadTooLarge,
        // Never format or log rejection sources: deserializers and body streams
        // can include the request payload or sensitive transport diagnostics.
        _ => {
            scafra_foundation::error!(
                error_kind = "request_body_error",
                "request body could not be read"
            );
            HttpError::Internal
        }
    };
    error_response(error)
}
