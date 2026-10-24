# Issue #12 implementation review

Status: **BLOCKED** on fresh authorization for the external consumer/negative
verification suites. The confirmed 401 defect is corrected. All seven standard
workspace checks, requested focused checks, Rust 1.88 check, two live generated
application variants and independent targeted review checks have passed on the
correction. No known source defect remains, but final verification is incomplete.
Historical verification below is not reused as proof of the corrected tree;
the final protocol section records the current results and remaining gate.

## Architecture decision

Keep the existing `{code,message}` JSON envelope and `application/json` as the
single default. The [canonical guide](../http-errors.md) records the compatibility
matrix prepared before implementation, examples, migration and exceptions.

Compared with RFC 9457, this retains existing clients and Rust/Axum ergonomics
without introducing URI problem types, a media-type migration or a legacy mode.
Problem Details has better standardized client/tool interoperability; retaining a
custom schema is a deliberate trade-off. Neither design inherently improves
redaction or performance. There is no requirement for instance URIs, arbitrary
extensions, or request metadata in current framework errors. Applications may
implement their own Axum response representation.

A non-exhaustive `scafra_foundation::http_error::HttpError` enum owns status,
machine code, safe explanation and serialization. Foundation already uses Serde;
it gains no HTTP, Axum, Tokio, or other dependency. Numeric status codes keep the
boundary independent of an HTTP runtime. Web and security already depend on
foundation. Actuator gains one first-party dependency on foundation, avoiding a
cycle through web/security or an additional crate. Three tiny Axum adapters apply
the shared mapping and `Json` serialization. They contain no independent schema,
status classification, or message mapping. Protocol challenges stay at their
own authentication boundaries.

Existing `AppError` is sufficient for typed applications. It remains the Axum
`IntoResponse` adapter and facade `Result` error. No global registry, conversion
trait, blanket `Display` conversion, or competing `ApiError` hierarchy is added.
Application-owned errors implement ordinary `IntoResponse` and explicit `From`.

## Before and after / public API

- `AppError::{NotFound,Internal}` and JSON rejection statuses, fields and safe
  messages remain compatible. The final protocol correction requires an explicit
  challenge payload on `Unauthorized`; see the final protocol review below.
- `AppError::Validation(String)` retains its payload for private inspection but
  now emits `request validation failed` (400). This intentional security default
  is a behavioral break for callers previously publishing arbitrary strings.
- Add `AppError::Forbidden` (403), `PublicValidation(String)` and
  `public_validation(...)` (400). Exhaustive enum matches need updates.
- Add public neutral `foundation::http_error::HttpError`, including
  `status()`, `code()`, `message()` and `Serialize`. Its enum is non-exhaustive.
  The facade's existing `foundation` re-export exposes the same type; no new
  feature flags, root re-exports or mandatory HTTP dependencies are added.
- Visible security 401/403, independent actuator 401 and processing 504 now
  contain the shared JSON contract instead of an empty body. Actuator gains a
  Bearer challenge. Application security advertises each configured scheme and
  no longer fabricates Basic for missing security configuration.
- Existing AppError/web/facade paths, HTTP `Result` alias, constructors and
  serving signatures remain. Workspace package versions remain 0.1.0.

These are pre-1.0 compatibility decisions requiring maintainer review before
freeze. Issue #16 is closed on GitHub at inspection time, but the checkout's
policy explicitly remains proposed; this change does not approve that policy.

## Security and protocol review

Route selection, principal construction, verifier caches, role/scope enforcement,
public overrides, nested guards, inventory adapters and finalized serving are
unchanged. Serializing denials does not authenticate anyone. Invalid configuration
still rejects standard startup; direct calls fail closed. JWT invalidity does not
fall back to another authenticator.

Visible 401s advertise configured Bearer and/or Basic with separate
`WWW-Authenticate` fields; 403s do not carry challenges. Actuator enforces its own
Bearer configuration and rejects duplicate Authorization values before evaluating
health checks. The final protocol correction makes `AppError::Unauthorized`
carry an explicitly selected typed challenge, so its conversion always emits the
required authentication header without context lookup.

Hidden denials remain empty 404 without media type, challenge or error code.
Native Axum fallback metadata and selected-route metadata can differ before
HTTP serialization; do not interpret the in-memory Content-Length difference as
a claim of universal header equality. The live harness compares HTTP/1 headers
except transport Date. Custom fallbacks/HTTP/2 embedding own their concealment
alignment; this change does not rewrite them.

AppError internal and default validation context, raw parser errors and stream
errors never enter public bodies. Scafra logs fixed internal classification and
configured backtrace diagnostics, never the supplied diagnostic string. Public
validation is an explicit opt-in to publishing reviewed text. Raw AppError
Display/Debug still contains private diagnostics for application inspection; user
code must not log or serialize it indiscriminately. Debug assertions do not enable
public detail. Sentinel regression tests capture TRACE logs and verify bodies.

A replacement async timeout layer preserves the old 504 status, optional default,
request-future cancellation and deadline-first polling when both futures are
ready. Independent review identified an initial use of Tokio's inner-first timeout;
that was corrected to biased deadline-first selection with a paused-time test.
Response streaming and non-yielding code are not newly bounded.

## Runtime integration and deliberate exceptions

| Framework boundary | Coverage |
| --- | --- |
| AppError absent/input/authentication/authorization/internal | shared mapping and Axum IntoResponse |
| JsonBody syntax/data/missing or unsupported content type/limit/stream failure | shared mapping; raw rejection sources discarded |
| Missing/disabled/invalid context or invalid policy/config | final protocol correction: generic500 (known hidden404 retained); no invented scheme |
| Configured credential denials | shared401 plus actual configured challenge(s) |
| Missing role or scope | 403 shared payload; enforcement unchanged |
| Actuator authentication | shared401 plus independent Bearer challenge; invalid server token configuration500 |
| Request-processing timeout | shared 504; same cancellation/deadline ordering |
| Unknown routes / unsupported methods | native Axum 404/405, Allow and custom fallbacks preserved |
| HEAD / successful no-content | native Axum handling, no blanket rewriting |
| Native Json and third-party handlers/extractors | ordinary Axum behavior preserved |
| Unhealthy/readiness/liveness health result | existing health document/status, including 503 |
| Startup/configuration/lifecycle failures | typed non-HTTP errors, outside request-response contract |
| BootUI | existing dashboard/HTML/access-control behavior |

## Performance and dependencies

Serialization happens only when constructing an error. Fixed explanations borrow
static strings; public validation moves its owned string. Success does not
construct an HttpError or allocate an error body. There is no shared mutable
registry, lock, dynamic type lookup, response buffering middleware, or repeated
serialization. The timeout replaces an existing layer with Axum async middleware;
it retains a timer and introduces Axum's ordinary boxed future dispatch. No
benchmark was run and no performance improvement or measured cost is claimed.

Actuator adds `scafra-foundation` as a normal dependency, reflected in Cargo.lock;
its direct consumer gains foundation's logging/diagnostics dependencies. The
facade and web already depend on foundation, so their normal dependency package
sets do not gain a new runtime dependency. Web adds Tokio `test-util` only as a
dev feature for the deadline regression. Foundation/core/config remain free of
new HTTP dependencies; no JWT/security dependency is added to web-only builds.

## Verification

Baseline: branch `12-standardize-http-api-error-responses`, initially clean at
`aee76e9a338e0c6d06a875b66ee88b9f168fdd67`. No branch change, commit, push,
remote mutation, package publication or version change was performed. Local
compiler: rustc/cargo 1.99.1; declared MSRV checked with rustc 1.88.0.
Workspace tests ran with `TMPDIR=$PWD/target/issue12-tmp` to keep scratch writes
inside the checkout and avoid small tmpfs limits.

### Final command outcomes

All commands below were executed and returned exit code 0. Initial failures and
corrections are recorded separately; these are the final outcomes.

| Command | Outcome |
| --- | --- |
| `cargo fmt --all -- --check` | PASSED |
| `cargo check --workspace` | PASSED |
| `cargo test --workspace` | PASSED; repeated on final source |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASSED |
| `cargo check --workspace --all-features` | PASSED |
| `cargo test --workspace --all-features` | PASSED; repeated after final source/test changes |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| `cargo test -p scafra-web --no-default-features` | PASSED |
| `cargo test -p scafra-web --all-features` | PASSED |
| `cargo test -p scafra-security --all-features` | PASSED |
| `cargo test -p scafra --features standard` | PASSED |
| `cargo test -p scafra --no-default-features --features web --test http_errors` | PASSED |
| `cargo test -p scafra --no-default-features --features security --test http_errors` | PASSED |
| `cargo test -p scafra --no-default-features --features jwt --test http_errors` | PASSED |
| `cargo test -p scafra --no-default-features --features actuator --test http_errors` | PASSED |
| `cargo +1.88.0 check --workspace --all-features --locked` | PASSED |
| `cargo +1.88.0 test -p scafra --features standard --test http_errors --locked` | PASSED |
| `cargo test -p scafra-cli --test generated_projects generated_projects_serve_routes_and_shutdown_gracefully -- --ignored --exact` | PASSED; explicitly ran the normally ignored loopback test |
| `python3 scripts/check-api-policy.py` | PASSED |
| `python3 -m unittest discover -s scripts -p test_api_policy.py` | PASSED, 16 tests |
| `python3 scripts/arch103-publish-order.py --check` | PASSED, all 12 packages ordered |
| `python3 -m py_compile scripts/issue12-http.py` | PASSED |
| `git diff --check` | PASSED |

The workspace test commands include macro trybuild, graph compile-fail/pass,
security-negative, nested route, JWT/JWKS, configuration, redaction, operational
health, lifecycle and generated-project coverage. Some existing documentation
examples remain marked ignored by their original test definitions; the existing
CLI live test was explicitly run rather than inferred from the workspace result.

Local logs: `target/issue12-verification/`,
`target/issue12-final-verification/results.json`,
`target/issue12-workspace-all-final.log`, `target/issue12-workspace-default-final.log`,
`target/issue12-standard-rerun.log`,
`target/issue12-cli-live.log`. These ignored logs are local inspection artifacts,
not committed evidence or published-package verification.

### New error matrix regression coverage

`crates/scafra/tests/http_errors.rs` uses actual procedural-macro controllers,
application-owned `IntoResponse` errors, explicit managed finalization, nested
protected controllers, public overrides and native extractors/fallbacks. It
asserts status, media type, exact JSON structure, stable code and safe explanation
for application absent/validation/internal/auth/forbidden errors, typed public
validation, malformed JSON, wrong JSON shape, missing/wrong media type, oversized
body, failed body stream and processing timeout. It captures TRACE diagnostics
and excludes sentinel private context, parser payloads, bearer/Basic/JWT data and
signing secrets. The test subscriber is process-wide within this integration-test
executable to avoid local tracing interest-cache races between parallel tests.

Security cases prove missing/invalid credentials, invalid signed JWT, missing
role and missing scope, configured challenge selection, nested public overrides,
HEAD, and empty hidden denials. Existing JWT/JWKS negative tests additionally
cover signature/algorithm/issuer/audience/expiration/key lookup and cache failure.
Web tests retain exact pre-existing JSON rejection bytes and add a paused-clock
deadline tie regression. Actuator tests now require JSON/challenge headers for
all aliases, reject duplicate credentials before health checks, and retain the
existing unhealthy/readiness document tests.

### Independent review

A read-only independent reviewer acting as `scafra-security-reviewer` reviewed
source, API/dependency boundaries, negative paths and tests. It identified the
initial timeout ordering regression, which was corrected and tested. Follow-up
review confirmed the actuator header/duplicate tests and standard/BootUI fixture
correction. The reviewer independently ran
`cargo test -p scafra --features standard --test http_errors --test config_resolution`:
all nine tests passed. It did not claim to run the live harness itself; the live
observations below were executed by the primary agent.

Required skill bodies were missing from `.agents/skills` in this checkout.
Read-only copies were located under
`/home/steffenp/.local/share/Trash/files/.agents/skills` and used for design,
Rust, security, API compatibility, macro-consumer, performance and verification review.
No outside-repository file was restored or modified.

### Actual generated applications over HTTP

The reproducible harness `scripts/issue12-http.py` creates a separate `[workspace]`
consumer manifest with path dependencies, its own offline-resolved lockfile,
normal Cargo tree and `#[scafra::main]` discovered controllers from
`scripts/fixtures/http-errors/`. These are actual downstream Cargo dependency
graphs; storing them under ignored `target/` does not make them workspace members.
They are source/path consumers, not published registry or pinned-Git evidence.

Executed commands (all PASSED):

```bash
python3 scripts/issue12-http.py target/issue12-live-standard-final --target-dir target/issue12-live-standard/target
RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/issue12-http.py target/issue12-live-web-msrv --features web
python3 scripts/issue12-http.py target/issue12-live-security --features security --target-dir target/issue12-live-standard/target
python3 scripts/issue12-http.py target/issue12-live-jwt --features jwt --target-dir target/issue12-live-standard/target
python3 scripts/issue12-http.py target/issue12-live-actuator --features actuator --target-dir target/issue12-live-standard/target
python3 scripts/issue12-http.py target/issue12-live-all --features all --target-dir target/issue12-live-standard/target
```

Outputs must be new directories when reproducing. Each retains sources, lock,
`tree.log`, `build.log`, per-mode process logs and `results.json`. Feature selections
resolve independently even where they share build artifacts. `all` selects
`standard,bootui`; it does not enable a dashboard listener in this error fixture.

Observed live behavior: application 404/400/500, JSON 400/422/415/413, safe typed
400, processing 504, configured 401 challenges, role/scope 403, native Json 400,
HEAD with empty body, authorized native 404/405 with Allow, independently secured
actuator health/readiness, and hidden 404. Hidden denial versus unknown-route
status/body/full HTTP/1 headers matched, excluding only transport Date. Global
security runs before native fallbacks, so 404/405 checks supply valid credentials
when required. Every started mode became ready within 15 seconds, accepted actual
loopback requests, and exited successfully within 15 seconds after SIGTERM. On
failure the harness kills and reaps its child. TRACE process logs excluded all
supplied sentinel secrets. Standard ran normal, hidden and JWT modes; each other
feature ran the applicable modes. Body-stream fault injection remains in-process
because a broken network request may be rejected by the HTTP parser before the
extractor can return a response.

The MSRV web consumer also asserts its normal tree excludes security, actuator,
scheduler, jsonwebtoken and reqwest. Other live feature trees are retained for
inspection. The mandatory existing comprehensive suites were subsequently run
successfully as recorded below.

### Early failures and fixes

- Initial Clippy: redundant actuator closure; changed to `.then(unauthorized)`;
  final default/all-feature Clippy passed.
- Initial new standard integration fixture: unrelated linked protected inventory
  caused disabled-security startup rejection. The fixture now explicitly finalizes
  only installed public-controller metadata. It never disabled a real guard.
- Initial default-web log capture was intermittent under parallel local
  subscribers. A single integration-process subscriber fixed capture coordination;
  final feature matrix and MSRV capture tests passed with the positive log assertion.
- Initial new concealment test compared Axum in-memory headers universally.
  Selected-route Content-Length differs from the native fallback fast path.
  In-memory tests now check privacy-bearing headers, and the actual HTTP/1 harness
  independently compares full headers except Date. Custom embedding limitations
  are documented; no fallback rewriting or guard relaxation was introduced.
- Existing standard-only `config_resolution` test failed because it unconditionally
  configured BootUI, which standard excludes. The test now retains BootUI snapshot
  assertions when selected and verifies configured server/401/challenge/redaction
  otherwise. Standard, all-feature and independent targeted reruns passed.
- Initial live fixture expected native 404/405 before global authentication. It
  now authenticates those fallback probes, retaining actual security precedence.

### Authorized final consumer verification — PASSED

After the user granted explicit scratch authorization, both previously blocked
suites were executed against the current working tree with Rust 1.88.0 and
returned exit code 0:

```bash
RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/arch103-consumers.py /var/tmp/scafra-issue12-verification-j59b1h02/consumers --runtime --timeout 600 --target-dir /var/tmp/scafra-issue12-verification-j59b1h02/target
RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/arch103-negative.py --output /var/tmp/scafra-issue12-verification-j59b1h02/negatives --target-dir /var/tmp/scafra-issue12-verification-j59b1h02/target --timeout 600
```

Both scripts explicitly reject checkout-local output paths. The user provided
the scoped authorization required by AGENTS.md and the verification skill. A new
root, `/var/tmp/scafra-issue12-verification-j59b1h02`, holds all new artifacts,
compiler scratch files and preserved logs. No unrelated outside-repository files
were modified, overwritten or removed. The existing suites ran without bypassing
or weakening their checks.

| Suite | Outcome | Duration | Evidence |
| --- | --- | --- | --- |
| Isolated consumers | PASSED, 16/16 cases | 84.92 seconds | `consumers-suite.log`, `consumers/results.json`, per-case build/tree/runtime logs |
| Negative verification | PASSED, 9/9 cases | 29.01 seconds | `negatives-suite.log`, `negatives/results.json`, `negatives/environment.json`, per-case compiler/runtime logs |

All evidence paths above are under the new root. `suite-results.json` preserves
the exact commands, Rust toolchain, exit codes and elapsed times. `working-tree.json`
records the branch, baseline commit and SHA-256 hashes of tracked and new source
files at suite start. A subsequent integrity check confirmed only this review
document changed while the suites ran; implementation, test, fixture and manifest
files were identical to those exercised. No source fixes or affected-test reruns
were needed because neither suite had an unexpected failure. Expected nonzero
compiler/process exits in negative cases were checked by the harness and are
successful rejection evidence, not suppressed failures.

Consumer cases: `minimal`, `config`, `core`, `foundation`, `web`, `secure-web`,
`inventory`, `jwt`, `nested`, `jwks`, `actuator`, `scheduler`, `default`, `standard`,
`bootui`, and `all`. All lock generation, builds and normal dependency-tree checks
passed with no forbidden packages. Runtime checks verified logging, actual
loopback requests, missing/invalid credentials, role/scope enforcement, nested
public overrides, inventory adapters, invalid JWT signatures, independent actuator
guards, readiness configuration rejection, BootUI snapshot redaction, scheduler
execution, resource cancellation/shutdown/drop and bounded SIGTERM termination
where applicable.

Negative cases: `main-requires-web`, `controller-requires-web`,
`protected-route-fails-closed`, `supplied-router-protection-fails-closed`,
`managed-resources-require-options-owner`,
`managed-resources-require-compatibility-owner`,
`unified-subcrate-capabilities-stay-rejected`,
`unified-jwks-cannot-enable-facade-jwt`, and
`features-are-additive-across-aliased-helper`. All nine passed. Unavailable module
and protected-router configurations were rejected before any listener was observed;
resource cleanup and expected diagnostics matched their assertions. The helper
feature-union case verified both the required normal dependencies and Cargo
feature forwarding.

These results close the prior verification gap. They supplement the workspace,
security redaction, generated-controller error matrix, MSRV and dedicated live
HTTP results above; all 20 acceptance criteria were reassessed against the
combined evidence. No required local check remains blocked or unexecuted.

`cargo package`/staged-registry verification is NOT RUN: this is not a package
release task. Publish dependency order was checked; no claim of published package
compatibility is made.

## Acceptance criteria

| Criterion | Status | Evidence |
| --- | --- | --- |
| One documented default HTTP error contract | PASS | canonical guide and shared HttpError serialization |
| Consistent status and media-type semantics | PASS | matrix, exact JSON tests and live HTTP |
| Ergonomic typed application conversion | PASS | AppError and application-owned DomainError in generated controllers |
| JSON parsing and validation contract | PASS | existing byte-exact tests plus generated and live matrix |
| Authentication status and configured challenges | PASS | final correction: typed standalone challenges, middleware matrix, real HTTP GET/HEAD and setup500 tests |
| Authorization remains fail-closed | PASS | existing security negatives, nested role/scope and actual 403 |
| Hidden-denial privacy | PASS | unchanged empty404; full live HTTP1 equality; custom embedding limits documented |
| Internal details not exposed publicly | PASS | Internal constructor/variant, parser/stream/validation sentinel bodies/logs |
| Timeout and body-limit representation | PASS | 504/413 tests, deadline tie test and live requests |
| Actuator health semantics | PASS | existing health/readiness tests and independent live actuator guard |
| Axum interoperability | PASS | native Json/fallback/405 Allow/custom response and HEAD tests |
| Minimal web-only dependency isolation | PASS | MSRV isolated normal tree excludes optional modules/security clients |
| Public API and migration documentation | PASS | canonical guide, upgrading, policy inventory and this review |
| Unit tests pass | PASS | default/all-feature workspace and focused tests |
| Integration tests pass | PARTIAL | current workspace, protocol matrix and live web/standard pass; existing isolated/negative suites need fresh authorization for this correction |
| Actual generated application over HTTP | PASS | six isolated feature consumers, macro main/discovery, bounded SIGTERM |
| Security redaction regressions | PASS | captured TRACE/body sentinels, existing config/JWT/JWKS tests and live logs |
| Rust 1.88 compatibility | PASS | locked all-feature workspace, controller tests and standalone web live consumer |
| Documentation matches implementation | PASS | response bytes/mappings and compiled equivalent controller/conversion examples; README timeout corrected |
| No relevant regression unresolved | PARTIAL | no known source defect after independent review and current local tests; final clearance awaits fresh existing isolated/negative verification |

Initial local assessment was READY FOR PR REVIEW; it is superseded by the
confirmed 401 protocol finding above.
Hosted CI and maintainer review remain separate from local completion; this is
not authorization to merge, publish, or close the issue.

## Remaining scope

Issue #19 still owns configurable/raw-body limits, default processing deadlines,
route override policy, and connection/header timeout ownership. This issue keeps
the existing 1 MiB extractor default and optional request-processing timeout.
Hosted CI, maintainer compatibility approval, release packaging/publishing and
final 1.0 freeze remain separate from local PR readiness.

## Final HTTP protocol correction for PR #110

Reviewed baseline: `e8dc45efa97ee13a05bba2c273a656754178e80c` on the existing
`12-standardize-http-api-error-responses` branch, initially clean. The previous
all-PASS assessment did not establish standalone 401 compliance. It was withdrawn
when this review confirmed the missing mandatory challenge.

### Root cause and reproduction before implementation

The unit `AppError::Unauthorized` mapped directly to the neutral `HttpError`
payload without attaching authentication metadata. Middleware had scheme context,
but missing-context and invalid-setup branches also reused that credential-error
classification. Earlier integration assertions verified status and JSON without
checking the challenge, and documentation shifted the header obligation to the
application despite a public standalone conversion.

RFC 9110 section 11.6.1 requires an applicable `WWW-Authenticate` challenge on a
401 ([reference](https://www.rfc-editor.org/rfc/rfc9110.html#section-11.6.1)). Merely
documenting that applications should repair the header was insufficient.

Three regressions were added and run before modifying production code. Each
returned test exit code 101 for the expected compliance failure. Logs and exact
commands are preserved in `target/issue12-protocol-repro/results.json` and
`target/issue12-protocol-repro/{0,1,2}.log`:

```bash
cargo test -p scafra-web --no-default-features direct_application_authentication_error_requires_a_challenge
cargo test -p scafra --no-default-features --features web --test http_errors public_generated_authentication_error_requires_a_challenge
cargo test -p scafra-security --test route_local missing_security_context_is_a_server_failure_not_challengeless_authentication
```

The first two verified the existing 401, media type and exact JSON, then failed
because the challenge was absent. The third demonstrated that a missing server
security context was incorrectly treated as a challenge-less 401 rather than a
server setup failure. These are intentionally retained failing baseline results;
they must not be reported as successful checks of the reviewed commit.

### Smallest sound solution and alternatives

Use explicit typed metadata for application-owned authentication and the existing
context for framework-owned authentication. `AppError::Unauthorized` now requires
an `AuthenticationChallenge` payload. Its private `HeaderValue` cannot be
constructed unchecked through public fields. `bearer()` emits Bearer;
`basic(realm)` quotes and escapes the realm and validates header bytes with
`HeaderValue`, returning `InvalidHeaderValue` for invalid bytes such as CR/LF.
Basic realm validation belongs at application startup; realms must not contain
credentials or request-supplied data. Only Scafra's supported Basic and Bearer
constructors are added. Custom schemes remain ordinary application-owned Axum
`IntoResponse`, avoiding a speculative general challenge parser or registry.

A global response-repair layer would guess schemes for public handlers or rewrite
unrelated responses. A hardcoded Bearer default would advertise an authenticator
that might not exist. Mapping genuine authentication failure to 403 would confuse
identity and authorization. Retaining a unit variant with a 500 fallback would
silently change valid application authentication failures. The small pre-1.0
payload change instead requires callers to make the applicable scheme explicit
at compile time while preserving the intended 401 wire classification.

Middleware derives challenges only from actual validated configuration. Missing,
disabled or invalid context and invalid policy metadata are server setup failures:
return a generic 500 `internal_error`, preserving known hidden404 settings. The
401 construction path also defensively checks that a challenge was attached.
Actuator's enabled-but-missing/blank token configuration similarly returns a
safe500; genuine rejected credentials retain its independent Bearer401. Standard
router/startup validation continues to reject invalid configuration before bind.

### Before/after protocol and API contract

| Path | Reviewed commit | Corrected contract |
| --- | --- | --- |
| Public application authentication error | Unit variant, 401 JSON, no challenge | `Unauthorized(AuthenticationChallenge)`, same401 JSON plus explicit applicable challenge |
| Missing/invalid/expired configured credentials | 401 and configured challenge(s) | unchanged |
| Missing route/handwritten security context | challenge-less401 | generic500, no challenge, handler not run |
| Invalid security config / disabled required security | 401, sometimes no or inapplicable challenge | generic500; known hide setting still empty404 |
| Invalid authorization metadata | credential-style401 | generic500; known hide setting still empty404 |
| Valid principal missing role/scope | 403, no challenge | unchanged |
| Hidden credential/authorization denial | empty404, no challenge | unchanged |
| Actuator missing/blank server token | 401 plus Bearer | generic500, no challenge, checks not run |
| Actuator rejected client token with valid config | 401 plus Bearer | unchanged |
| HEAD authentication failure | incomplete standalone headers | 401 plus applicable challenge and no body |

`AuthenticationChallenge` is exported by `scafra_web`, `scafra::web`, the web-enabled
facade root and prelude, under the same feature boundary as `AppError`. Replace
`AppError::Unauthorized` with
`AppError::Unauthorized(AuthenticationChallenge::bearer())` for an implemented
Bearer authenticator, or the fallible Basic constructor. Exhaustive matches use
`Unauthorized(_)`. This is a deliberate pre-1.0 Rust source break and a correction
to invalid-setup HTTP status/code behavior; workspace versions remain unchanged.
There are no manifest/dependency/feature changes, no HTTP dependencies in foundation,
no mandatory security dependencies for web-only consumers, and no global state.
Successful request processing is unchanged. The payload format and redaction,
route selection, credential verification, role/scope checks, public overrides,
legacy inventory behavior, health documents, JSON errors, timeouts and native
Axum interoperability are preserved.

### Independent correction review

An independent read-only security reviewer inspected the targeted diff and found
no confirmed correctness or security defect. It independently executed:

- `cargo test -p scafra-web --no-default-features challenge` — PASSED, two tests.
- `cargo test -p scafra-security --test route_local --test middleware` — PASSED,
  14 tests including real HTTP setup-failure checks.

The reviewer did not independently run the Python live harness or full workspace
matrix. Primary-agent verification results are recorded separately below.

### Correction verification and final reassessment

All commands in this table returned exit code 0 on the corrected source.
Exact exit codes, durations and logs are retained in
`target/issue12-protocol-checks/results.json`:

| Current correction command | Outcome |
| --- | --- |
| `cargo fmt --all -- --check` | PASSED (`00.log`) |
| `cargo check --workspace` | PASSED (`01.log`) |
| `cargo test --workspace` | PASSED (`02.log`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASSED (`03.log`) |
| `cargo check --workspace --all-features` | PASSED (`04.log`) |
| `cargo test --workspace --all-features` | PASSED (`05.log`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED (`06.log`) |
| `cargo test -p scafra-web --no-default-features` | PASSED (`07.log`) |
| `cargo test -p scafra-web --all-features` | PASSED (`08.log`) |
| `cargo test -p scafra-security --all-features` | PASSED (`09.log`) |
| `cargo test -p scafra --features standard --test http_errors` | PASSED (`10.log`) |
| `cargo +1.88.0 check --workspace --all-features --locked` | PASSED (`11.log`) |
| `python3 scripts/check-api-policy.py` | PASSED (`12.log`) |
| `python3 -m unittest discover -s scripts -p test_api_policy.py` | PASSED (`13.log`) |
| `cargo test -p scafra --no-default-features --features web --test http_errors` | PASSED (`14.log`) |
| `cargo test -p scafra-security --no-default-features` | PASSED (`15.log`) |

Additional focused run:
`cargo test -p scafra-web -p scafra-security -p scafra-actuator --all-features`
PASSED (`target/issue12-protocol-repro/focused.log`). An initial invocation
repeated `--all-features` and Cargo rejected its command line before compiling;
the corrected invocation above passed. This was not a source/test failure.

The existing live harness was extended with a public generated controller
returning explicitly selected Bearer and Basic errors. These actual generated
applications both PASSED, including status, exact JSON, applicable challenges,
HEAD empty-body semantics, normal failures, limits, timeouts, native Json and
Allow handling and bounded SIGTERM shutdown:

```bash
python3 scripts/issue12-http.py target/issue12-protocol-live-web --features web --target-dir target/issue12-live-standard/target
python3 scripts/issue12-http.py target/issue12-protocol-live-standard --target-dir target/issue12-live-standard/target
```

Preserved logs: `target/issue12-protocol-live-web.log`,
`target/issue12-protocol-live-standard.log`; each output directory contains its
own sources, lockfile, normal dependency tree, process logs and `results.json`.
These are independently resolved source/path consumers, not published artifacts.
The web-only tree asserts absence of security/JWT/actuator/scheduler/reqwest.
Standard additionally verifies Basic/Bearer middleware challenges, role/scope403,
public overrides, invalid JWT401, hidden404 full HTTP/1 header equality (excluding
Date), independent actuator security and health. Existing workspace regressions
cover nested controllers, route-local policies, legacy inventory adapters,
JWT/JWKS, configuration redaction, JSON validation and 504 timeout behavior.

`security_setup_failures_are_protocol_correct_over_http` also serves actual
loopback HTTP for GET and HEAD against route-local and handwritten guards without
context, invalid legacy/context configurations, disabled required security,
contradictory metadata and known hidden invalid configuration. Assertions require
generic JSON500 without challenge or leaked sentinel/private handler data, or an
empty404 for concealment. Read and shutdown operations have five-second bounds.

Fresh external suite commands are **NOT RUN (BLOCKED)**. They reject checkout-local
outputs. Because this corrective assignment explicitly says not to assume prior
authorization covers new external paths, fresh approval was requested for newly
created `/var/tmp/scafra-issue12-protocol-*` directories. No answer has been
received, and no new external path was written during this corrective review.
After approval, rerun both `scripts/arch103-consumers.py --runtime` and
`scripts/arch103-negative.py` on Rust 1.88.0, preserving their results under that
new prefix. Prior 16/16 and 9/9 results above remain historical evidence for the
initial implementation, not current verification of this correction.

The 20 acceptance criteria were reassessed: 18 PASS and two PARTIAL (integration
coverage/final regression clearance pending the fresh suites). Standalone public
authentication compliance itself is PASS, with the root cause fixed and tested;
there is no assertion that an incomplete review has all 20 PASS. Final status is
**BLOCKED** on the verification permission gate, rather than READY FOR MERGE REVIEW.
Hosted CI and maintainer approval remain separate. No commit, push, merge, PR
creation, package publication or issue closure was performed.
