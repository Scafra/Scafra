# Upgrading Scafra

Read the [compatibility policy](api-stability.md) and the release notes for both
the old and target release. Current 0.1.0 is pre-1.0; stable candidates are not
already covered by a shipped 1.0 guarantee. No deprecated API or compatibility
shim is introduced by this guide.

## Pre-modular to modular capability selection

A former bare `scafra = "0.1"` dependency no longer implies HTTP, security,
JWT/JWKS, actuator or scheduling. Start with this valid Cargo manifest entry:

```toml
[dependencies]
scafra = { version = "0.1", default-features = false, features = ["standard"] }
```

`standard` selects web/security/jwt/jwks/actuator/scheduler. Add `bootui`
explicitly for the dashboard. This restores capabilities, not every previous
source signature or behavior. Reduce the list to actual needs using the
[module feature table](modules.md#cargo-feature-reference); web-only applications
can select `web`, while scheduler-only applications supply their own runtime.
There is no supervised worker entrypoint yet.

## Source and behavior changes to inspect

- `ScafraConfig` integration fields and `RouterOptions` fields depend on compiled
  features; `StartupError` module variants and root/prelude exports do too.
  Use constructors/default updates where appropriate and conditional code in
  libraries supporting multiple selections. Match the resolved feature set
  when reviewing exhaustive matches; optional config sections are rejected in
  validated framework loading even if their `enabled` flag is false.
- `ConfigLoader::load_validated::<ScafraConfig>()` activates framework namespace
  and capability checks. Raw domain `load<T>()` follows the domain's schema.
  Do not bypass validation to suppress unavailable-security errors. Remove an
  intentionally unused section or enable and configure its required capability.
- Import HTTP extractors/errors from a web-enabled facade/prelude or
  `scafra::web::axum`. Direct config/security/web dependencies need their own
  features; they do not select facade features in reverse. Check `cargo tree
  -e features` for unification, and `cargo tree -d` for duplicate type identities.
- Rebuild graph artifacts from source with matching `scafra-build` and facade
  releases. Generated `compose`/`compose_with` are sync-only; async graphs use
  `compose_async`/`compose_async_with` and retain `Resources`. Override slots
  `node_N` reflect the current application graph; review them after provider
  changes. Providers return public application-owned structs/newtypes, not
  foreign resource aliases. `#[bean]` does not generate Bean/Default impls.
- Keep resource owners until serving ends; use
  `build_finalized_router_with_options_and_resources` and
  `run_with_finalized_resources` for the managed resource path. The generated
  main uses this ownership flow. Clean up successfully initialized resources
  if custom router construction fails. See [resource lifecycle](guides/resource-lifecycle.md).
- Generated controller policies follow actual selected handlers through nested
  composition. For custom composition, finalize once with
  `scafra::web::finish_managed_router` and serve via
  `scafra::run_with_finalized_router` or
  `scafra::web::serve_finalized_router_with_callbacks`. Guard handwritten Axum
  routers with `scafra::web::guard_axum_router` **before** merging if they should
  inherit application security. Supply complete route metadata for validation.
- Existing raw `finish_router_with_options` / raw serving APIs globally guard
  arbitrary routes. Re-finalizing opaque raw routers can repeat middleware and
  deny nested public overrides conservatively. `FinalizedRouter::nest` preserves
  the wrapper; `into_router` exposes guarded Axum interoperability but loses its
  finalization marker. Use finalized serving when preserving public overrides.
  Handwritten inventory adapters now register on isolated routers; adapters that
  mutate other adapters' routes need explicit application composition.
- Cargo security selection only compiles enforcement. Set runtime security and
  real credentials explicitly. Removing a capability must produce an error for
  protected/configured intent, never a public fallback. Test anonymous, invalid,
  valid, missing-role and missing-scope requests after changing composition.
- BootUI is separately selected and experimental, including `BootUiConfig`,
  its schema/re-exports and `ScafraConfig.bootui`; opting in can require updates
  to aggregate struct literals when those experimental fields change. Keep the
  effective config/profile
  handoff and local-only/peer protections; review allow-listed snapshots. Do not
  assume a stable dashboard JSON schema.
- CLI starters for web/api/service/monolith select `web`. Use `scafra new app
  --features security,actuator` or `--features standard,bootui` as needed.
  Generation does not supply secrets. Use a CLI matching the target runtime
  release; regenerate into a fresh directory for comparison and port intended
  changes to application-owned files. Never overwrite a modified application.

## Routine version upgrade

1. Read release notes, including experimental changes and migration examples.
2. Review feature/dependency/default changes and runtime configuration intent.
3. Update facade and direct companion/build dependencies together to the target
   coordinated release; regenerate emitted Rust and inspect Cargo.lock/tree.
4. Rebuild with the documented MSRV, currently `cargo +1.88.0 check --locked`,
   and current stable. The proposed policy permits reviewed MSRV increases in
   1.x minors, never patches: inspect the target release's compiler floor first.
   To remain on an older compiler, constrain the last compatible Scafra minor
   and retain a compatible lockfile; no backport guarantee follows.
   Clean consumers must resolve compatible dependencies on
   the MSRV too; do not assume a workspace lock governs downstream resolution.
5. Resolve compiler errors using the release migration notes. If a release
   actually marks an API `#[deprecated]`, use its warning and rustdoc replacement
   guidance, test the replacement, and remove old calls before the next major.
   There is no invented deprecation schedule for current APIs.
6. Run application unit/integration tests and a real generated-project startup.
7. Verify security denial/public exceptions, JWT claims/rotation, operational
   access, redaction and feature-union behavior under the effective configuration.
8. Verify readiness, provider failure rollback, scheduler/resource cancellation,
   request draining and reverse teardown on normal shutdown and SIGTERM on the
   tested Linux target. Cooperative callbacks/resources can delay shutdown;
   the HTTP drain deadline is not a universal process deadline.

For prereleases use explicit versions, preferably exact matching versions for
all Scafra packages. Keep the previous application revision and lockfile for a
reviewable rollback; rollback must not reintroduce a known security defect.

## Before the 1.0 freeze

Current error mappings and resource protection are implemented behavior, not a
final universal error/limit design. #12 must settle response/status/schema
contracts; #19 must settle configurable/raw-body limits, timeout defaults and
connection/header ownership. A fixed extractor limit does not protect every raw
body handler. Read their migration notes when completed and retest configuration,
router construction and error handling. #15/#33/#42 supply published-template,
compiler/target and RC/upgrade evidence before the final 1.0 inventory is frozen.
See the [candidate freeze gates](api-stability.md#candidate-freeze-gates-and-classification-exceptions).

## HTTP error contract before 1.0

Issue #12 retains `{code,message}` and `application/json`, and extends it to
visible authentication/authorization denials, actuator authentication, and
processing timeouts. Hidden 404, native Axum fallbacks/extractors and health
documents retain their existing behavior. `AppError::Validation(String)` now
redacts its string; use `AppError::public_validation("reviewed safe message")`
for deliberate public text. Exhaustive AppError matches must handle the new
`Forbidden` and `PublicValidation` variants. See [HTTP errors](http-errors.md)
for the status matrix, challenges, application-owned error conversions and
wire-format migration details.

The final HTTP protocol correction requires a challenge payload on the existing
variant: replace `AppError::Unauthorized` with
`AppError::Unauthorized(AuthenticationChallenge::bearer())` for an actual Bearer
authenticator, or use the fallible `AuthenticationChallenge::basic(realm)` for
Basic. `AuthenticationChallenge` is exported with `AppError` through web, facade,
and prelude. Match the variant as `Unauthorized(_)`. Invalid/missing/disabled
security context or invalid authorization metadata now produces a generic 500
instead of a credential-oriented 401; standard startup validation and known
hidden404 behavior remain intact. This is a deliberate pre-1.0 Rust/wire behavior
correction. All valid authentication 401s keep the `unauthorized` classification,
existing JSON and applicable challenges.

## Configuration and resource protection (CFG-01)

The coordinated 0.1.0 workspace remains pre-1.0. These changes intentionally
tighten deployment defaults without changing package versions. Review them
before upgrading an existing application.

- Omitted `server.timeout` changes from unlimited to 30 seconds. Use
  `server.timeout: null` explicitly for unlimited processing, or set 1–86400
  seconds. Connection inactivity remains independently bounded. The alias
  `server.request_timeout_seconds` is retained; duplicate aliases in one source
  now fail instead of depending on deserialization order.
- `ServerConfig`, `RouterOptions` and `JwtConfig` gain public fields;
  `ScafraConfig` gains `shutdown`. Update exhaustive literals with
  `..Default::default()` and use the canonical keys in the
  [reference](configuration-reference.md). Standalone ServerConfig validation
  now returns `ValidationError` rather than `Infallible`.
- The former 1 MiB extractor limit is also a global streaming ceiling. Raising
  a route extractor limit cannot remove it. Set an adequate global ceiling and
  tighten individual routes with `with_request_body_limit`. Both that helper
  and `with_request_timeout` return `Result` after validation. A longer local
  deadline cannot remove an outer timer.
- Managed serving now enforces header-read, transport inactivity, header
  byte/count and connection/HTTP2-stream budgets. HTTP/2 support adds `h2` to
  the web-only dependency closure. Finalized routers retain connection options;
  converting to raw Axum transfers connection-policy ownership to the embedding
  server. Existing serving signatures remain available.
- `ShutdownConfig::policy()` feeds normal and startup-failure cleanup. Generated
  graphs add `compose_async_with_policy(overrides, policy)` while preserving
  existing default-policy composition functions. Regenerate with matching
  crates; custom executors must still honor cooperative resource shutdown.
- Actuator checked constructors now reject invalid deadlines, endpoint names
  and blank enabled credentials. `HealthRegistryError` gains variants; review
  exhaustive matches. Legacy infallible routes still fail closed. BootUI
  rejects unsafe bind/auth and dynamic/invalid mount paths before nesting; its
  policy error gains `InvalidServerConfiguration`. The standard runner now
  owns its resource-protected listener. Standalone users can own
  `start_managed_with_config`'s `BootUiServer` and await shutdown. The legacy
  independent start helper uses its own signal boundary.
- JWT verification retains signature/algorithm/expiry enforcement. Cache and
  network timing and clock skew are configurable within validated bounds.
  Review rotation overlap when changing TTL or leeway.
- Development polling accepts `scafra dev --poll-interval-ms 750` or
  `SCAFRA_DEV_POLL_INTERVAL_MS=750` (100–60000 ms). An explicit flag wins.

No setting enables an unavailable Cargo feature. No live mutable configuration
service is introduced; restart after changing runtime settings.

Scheduler names keep their literal case and punctuation. Dotted properties
paths use the final task property as separator; environment names can use
`TASK_HEX_<exact-name-bytes>` for ambiguous names. Domain loaders are unaffected.
Use `ConfigLoader::profile` or `SCAFRA_PROFILE` for runtime profile selection;
late ordinary overrides of `scafra.profiles.active` now reject instead of being
ignored. Enabled Basic configuration must contain both username and password,
even when another authenticator is usable.

## CFG-01 authoritative loader hook

Existing entrypoints and runner signatures continue to work. Applications that
previously loaded a separate config for overrides should instead pass their
loader through `#[scafra::main(config_loader = crate::configured_loader)]`. For
manual startup, `run_with_loader` and `run_with_loader_and_resources` accept it
directly. This ensures that address, logging, scheduler, BootUI and shutdown
consume the same validated result as router construction. The hook returns a
`ConfigLoader`, runs once after the original entry body, and preserves the
existing source precedence. See the [reference](configuration-reference.md#equivalent-examples).

Unknown macro options now fail rather than being silently ignored. Loader
`Debug` output omits all override names and values; use typed assertions to test
configuration rather than inspecting diagnostic strings. Bind-failure cleanup
now requests resource cancellation before waiting for active scheduled jobs;
resource owners must continue to make cancellation requests idempotent.
