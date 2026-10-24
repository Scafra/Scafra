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
