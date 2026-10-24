# HTTP API errors

Scafra-owned API failures use one JSON envelope, with `Content-Type:
application/json`:

```json
{"code":"internal_error","message":"internal server error"}
```

Clients branch on HTTP status and `code`, not human-readable `message`. Codes
are stable classifications; messages are safe explanations and may improve.
Successful requests do not construct or serialize an error payload.

## Status, code and media-type compatibility matrix

This matrix was recorded before implementing issue #12. The selected contract
retains the existing application/JSON wire format. New structured denials and
processing timeouts replace previously empty bodies.

| Outcome | Status | Code | Before | Selected media type / behavior |
| --- | --- | --- | --- | --- |
| Application resource absent | 404 | `not_found` | JSON | application/json |
| Application validation | 400 | `validation_error` | JSON, raw string | application/json, generic default |
| JSON syntax | 400 | `invalid_json` | JSON | application/json |
| JSON shape/deserialization | 422 | `validation_error` | JSON | application/json |
| Missing/unsupported JSON content type | 415 | `unsupported_media_type` | JSON | application/json |
| JSON extractor body limit | 413 | `payload_too_large` | JSON | application/json |
| Body stream/buffering failure | 500 | `internal_error` | JSON | application/json |
| Authentication rejected | 401 | `unauthorized` | empty | application/json; configured challenges retained |
| Authenticated, missing role/scope | 403 | `forbidden` | empty | application/json |
| Hidden denial | 404 | none | empty | empty, no challenge or media type |
| Internal application failure | 500 | `internal_error` | JSON | application/json |
| Processing deadline | 504 | `request_timeout` | empty | application/json |
| Native unknown route | 404 | none | Axum default | Axum default (empty) |
| Native unsupported method | 405 | none | Axum default | Axum default, `Allow` retained |
| Unhealthy/not ready actuator | 503 | health document | health JSON | existing operational document |

Hidden denials intentionally retain empty 404 responses, matching native unknown
routes and avoiding a new resource-existence signal. Custom application fallbacks
remain the application's responsibility. HEAD responses have no body. Scafra
never rewrites arbitrary responses, native `axum::Json` rejections, custom
extractors, HTML dashboards, or operational health documents.

## Design decision

The existing envelope is the default, rather than RFC 9457 Problem Details
([specification](https://www.rfc-editor.org/rfc/rfc9457.html)). Problem Details
provides standard tooling interoperability, URI problem types, and extensibility.
Scafra currently needs only a status, machine classification and safe explanation;
its existing clients already consume `code` and `message`. Retaining those fields
avoids a media-type/field migration, URI taxonomy, and a second compatibility
format. Both approaches can be strongly typed, safely redacted, and serialized
only on error paths. Neither has a demonstrated performance advantage here.
Axum applications needing Problem Details can implement their own `IntoResponse`.
This is an explicit compatibility/interoperability trade-off, not a claim of RFC
9457 conformance.

## Returning errors from controllers

With the `web` feature, existing `scafra::AppError`, `scafra::web::AppError`,
`scafra_web::AppError`, the prelude export and `scafra::Result<T>` remain available.
No additional conversion trait or error hierarchy is required.

```rust
use scafra::prelude::*;

#[controller("/orders", public)]
struct Orders;

#[routes(default)]
impl Orders {
    #[get("/{id}")]
    async fn find(&self, id: Path<u64>) -> scafra::Result<String> {
        if id.0 == 0 {
            return Err(AppError::public_validation("order id must be positive"));
        }
        Err(AppError::NotFound)
    }
}
```

`Validation(String)` retains private diagnostic context and returns the generic
400 explanation `request validation failed`. It never automatically exposes the
string. `public_validation(...)` / `PublicValidation(String)` explicitly opt in
to publishing reviewed text. Use fixed messages; do not interpolate secret values,
raw payloads, parser errors, connection strings, paths, or error chains. Valid
JSON that fails deserialization returns 422; application validation retains its
existing 400 status. `JsonBody<T>` accepts Axum's JSON media types, including
`application/*+json`, and uses Axum's body buffering and limit behavior.

Application-owned error types implement ordinary Axum conversion:

```rust
use scafra::web::axum::response::{IntoResponse, Response};
use scafra::AppError;

pub enum OrderError { Missing, InvalidQuantity, Storage(std::io::Error) }
impl IntoResponse for OrderError {
    fn into_response(self) -> Response {
        match self {
            Self::Missing => AppError::NotFound,
            Self::InvalidQuantity => AppError::public_validation("quantity must be positive"),
            Self::Storage(error) => AppError::internal(error),
        }.into_response()
    }
}
```

A controller can return `Result<T, OrderError>` directly; `?` uses the
application's explicit `From` implementations. Scafra intentionally has no blanket
conversion from arbitrary `Display` text into public messages.

## Authentication and privacy

Security middleware returns 401 for missing, invalid or expired credentials and
403 for an authenticated principal lacking roles/scopes. These are independent
of response serialization: the route-local enforcement and managed/finalized
router model are unchanged. Invalid security configuration still fails closed;
standard startup rejects it before serving. Direct guard calls without a context
also deny access, without inventing a scheme.

A middleware 401 advertises Bearer when JWT/shared bearer is configured and
`Basic realm="Scafra"` when both Basic fields are configured. Both configured
schemes produce separate `WWW-Authenticate` fields. A 403 has no challenge.
The challenge is HTTP authentication metadata as specified by
[RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-11.6.1).
`AppError::Unauthorized` alone has no access to authentication configuration:
application-owned authentication must attach its configured challenge using
Axum response headers. It must not invent a scheme. Middleware normally owns
this boundary for protected Scafra controllers.

`hide_unauthorized` suppresses both denials to empty 404 without challenge,
code or authentication explanation. It preserves the existing concealment
behavior; applications with custom fallback content must maintain their own
uniform concealment policy. The framework does not rewrite those fallbacks.
Native Axum selected routes and fallback fast paths can differ in in-memory
Content-Length metadata. Scafra's HTTP/1 server normalizes empty responses on the
wire; tests compare full headers except Date. Custom embedding (including HTTP/2)
must assess its own fallback/header alignment.
Actuator has independent Bearer access control and now returns the same 401
payload with its own challenge. A valid unhealthy/readiness 503 remains a health
document. BootUI content and access control are unchanged.

Internal application context is retained in `AppError::Internal(String)` or
`AppError::internal(error)` and never copied into the response. Scafra logs a
fixed error classification and the configured backtrace diagnostic, without the
error's Display/Debug text. `Validation` context and JSON rejection sources are
not logged. Application code is responsible for safe logging if it explicitly
formats its own errors; the raw diagnostic variants still contain private data.
Debug builds do not change public exposure.

## Limits and interoperability

`JsonBody` reports 413 for Axum's extractor size limit and 500 for unexpected
body-stream/buffering errors. Framework finalization still installs a 1 MiB
extractor limit. A route-specific `DefaultBodyLimit` remains supported. These
are extractor limits, not a universal cap on raw body handlers.

Configured processing deadlines cancel the handler future and return 504 with
`request_timeout`. The deadline wins if both deadline and pending handler become
ready on the same poll, preserving the previous Tower semantics. It does not
bound response-body streaming, roll back external side effects, or preempt
blocking code. There is still no default processing deadline. Configurable raw
body protection, default deadlines and connection/header policies remain #19.

After applicable authentication/authorization checks, unknown routes and
unsupported methods retain native Axum 404/405, including `Allow`. Native extractors, user responses and status-bearing successful health
results remain untouched. Application fallbacks and application-specific error
codes can use ordinary `IntoResponse`; Scafra does not standardize every possible
Axum response. HEAD stripping is performed by Axum, including framework failures;
no-content responses are never passed through blanket error serialization.

## Migration before 1.0

The workspace is still 0.1.0. This implementation does not declare the proposed
[API stability policy](api-stability.md) approved or claim a shipped 1.0 contract.

- Existing application/JSON `code`/`message` fields and media type remain intact.
- `Validation(String)` now redacts its string. Replace deliberate safe messages
  with `AppError::public_validation("reviewed explanation")`.
- `AppError` gains `Forbidden` and `PublicValidation`; exhaustive enum matches
  need new arms. Existing error paths and the HTTP result alias remain intact.
- Security/actuator 401 and security 403 now contain JSON. Clients expecting empty
  bodies should parse the envelope. Hidden 404s stay empty.
- Actuator now sends a Bearer challenge; multiple configured application schemes
  are advertised independently. Unconfigured Basic is no longer advertised.
- Processing 504 now contains JSON. Native 404/405 and operational 503 remain as
  before. There is no competing legacy-format mode.

See the [implementation review](architecture/issue-12-review.md) for exact
verification commands, compatibility review and release follow-ups.
