//! Streaming request ceilings. No body is read ahead or buffered here.
use axum::{
    body::{Body, Bytes},
    extract::Request,
    middleware::{from_fn, Next},
    response::Response,
    Router,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use http_body_util::{LengthLimitError, Limited};
use scafra_foundation::http_error::HttpError;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

pin_project_lite::pin_project! {
    struct ObservedLimit {
        #[pin]
        inner: Option<Limited<Body>>,
        exceeded: Arc<AtomicBool>,
    }
}
impl HttpBody for ObservedLimit {
    type Data = Bytes;
    type Error = axum::BoxError;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let mut this = self.project();
        let Some(inner) = this.inner.as_mut().as_pin_mut() else {
            return Poll::Ready(None);
        };
        let frame = inner.poll_frame(cx);
        if let Poll::Ready(Some(Err(error))) = &frame {
            if error.is::<LengthLimitError>() {
                this.exceeded.store(true, Ordering::Relaxed);
            }
            // Body errors are terminal. Release transport ownership immediately
            // even when application code keeps polling after a rejected frame.
            this.inner.set(None);
        }
        frame
    }
    fn is_end_stream(&self) -> bool {
        self.inner.as_ref().is_none_or(HttpBody::is_end_stream)
    }
    fn size_hint(&self) -> SizeHint {
        self.inner
            .as_ref()
            .map(HttpBody::size_hint)
            .unwrap_or_else(|| SizeHint::with_exact(0))
    }
}
/// Adds a streaming ceiling to an Axum router, including raw body consumers.
/// Route-local ceilings can tighten but cannot remove an enclosing ceiling.
/// Unread bodies are not drained. Once response headers are sent, later body
/// errors terminate the stream and cannot change its status into HTTP 413.
pub fn with_request_body_limit(router: Router, limit: usize) -> Result<Router, crate::WebError> {
    validate_body_limit(limit)?;
    Ok(body_limit(router, limit))
}
pub(crate) fn validate_body_limit(limit: usize) -> Result<(), crate::WebError> {
    if !(1..=1_073_741_824).contains(&limit) {
        return Err(crate::WebError::Server(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "max_request_body_bytes must be between 1 and 1073741824 bytes",
        )));
    }
    Ok(())
}
pub(crate) fn body_limit(router: Router, limit: usize) -> Router {
    router.layer(from_fn(move |request: Request, next: Next| async move {
        enforce(request, next, limit).await
    }))
}
async fn enforce(request: Request, next: Next, limit: usize) -> Response {
    let declared = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if request.body().size_hint().lower() > limit as u64
        || declared.is_some_and(|length| length > limit as u64)
    {
        return crate::errors::error_response(HttpError::PayloadTooLarge);
    }
    let exceeded = Arc::new(AtomicBool::new(false));
    let (parts, body) = request.into_parts();
    let body = Body::new(ObservedLimit {
        inner: Some(Limited::new(body, limit)),
        exceeded: Arc::clone(&exceeded),
    });
    let response = next.run(Request::from_parts(parts, body)).await;
    if exceeded.load(Ordering::Relaxed) {
        crate::errors::error_response(HttpError::PayloadTooLarge)
    } else {
        response
    }
}
