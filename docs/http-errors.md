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
| Authentication rejected (configured/app-declared scheme) | 401 | `unauthorized` | empty | application/json; applicable challenge required |
| Missing/disabled/invalid security setup or invalid policy | 500 | `internal_error` | sometimes challenge-less 401 | application/json, no invented challenge; known concealment still empty404 |
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
standard startup rejects it before serving. Direct guards without an installed
context, explicitly protected routes with disabled security, invalid policies,
and invalid security configurations return a safe 500 `internal_error`. These
are server setup failures, not credential rejections. A known `hide_unauthorized`
setting retains empty 404 concealment even for setup failures. Without a context,
no concealment configuration or authentication scheme is guessed.

A middleware 401 advertises Bearer when JWT/shared bearer is configured and
`Basic realm="Scafra"` when both Basic fields are configured. Both configured
schemes produce separate `WWW-Authenticate` fields. A 403 has no challenge.
The challenge is HTTP authentication metadata as specified by
[RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-11.6.1).
Every supported Scafra-owned 401 includes an applicable challenge. An
application-owned authentication error must carry that choice explicitly:

```rust
use scafra::{AppError, AuthenticationChallenge};

fn authentication_failed() -> AppError {
    // Select Bearer only when the application's own authenticator implements it.
    AppError::Unauthorized(AuthenticationChallenge::bearer())
}
```

A public generated controller can return `Err(authentication_failed())` through
`scafra::Result<T>`. This works with web alone, direct Axum routers and finalized
routers; no security context lookup or response-rewriting middleware is involved.
Basic uses `AuthenticationChallenge::basic("Orders")`, returning a `Result` whose
error is Axum's `http::header::InvalidHeaderValue`. Validate configured realms at
startup: quotes/backslashes are escaped and invalid header bytes (including
CR/LF) are rejected. Challenge constructors neither install nor enable an
authenticator. Never use credentials or untrusted request data as a realm.
These two constructors cover Scafra's supported schemes. Applications using
custom authentication schemes can implement ordinary Axum `IntoResponse` with
their own applicable, validated challenge.

For example, the Bearer failure is:

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json
WWW-Authenticate: Bearer

{"code":"unauthorized","message":"authentication required"}
```

A HEAD request carries the same status and challenge with no body. The neutral
foundation `HttpError` is a serializable payload/classification, not a complete
HTTP authentication response; application-owned raw Axum responses must still
satisfy HTTP semantics. Scafra does not rewrite arbitrary application responses.

`hide_unauthorized` suppresses both denials to empty 404 without challenge,
code or authentication explanation. It preserves the existing concealment
behavior; applications with custom fallback content must maintain their own
uniform concealment policy. The framework does not rewrite those fallbacks.
Native Axum selected routes and fallback fast paths can differ in in-memory
Content-Length metadata. Scafra's HTTP/1 server normalizes empty responses on the
wire; tests compare full headers except Date. Custom embedding (including HTTP/2)
must assess its own fallback/header alignment.
Actuator has independent Bearer access control and now returns the same 401
payload with its own challenge. Missing or blank actuator credentials in the
server configuration instead return a safe 500 on direct router use (standard
startup rejects that configuration). A valid unhealthy/readiness 503 remains a health
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
- The protocol correction changes the unit variant `AppError::Unauthorized` to
  `AppError::Unauthorized(AuthenticationChallenge)`. Supply `bearer()` or a
  validated `basic(realm)` for the scheme actually implemented. Update patterns
  to `AppError::Unauthorized(_)`. Existing web/facade/prelude paths export the
  challenge type with the same `web` feature boundary.
- Direct security setup failures (including invalid actuator token configuration)
  now return 500 `internal_error` instead of 401 `unauthorized`. They remain
  fail-closed; genuine missing/invalid/expired credentials retain 401 with their
  configured challenges, and role/scope denial retains 403.
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
