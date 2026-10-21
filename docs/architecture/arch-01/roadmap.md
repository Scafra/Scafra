# Implementation roadmap after Phase 0

> Historical Phase 0 snapshot/planning evidence. The [implemented contract](../../modules.md)
> and [implementation results](implementation.md) supersede proposed/current-state claims below.

Execution update: all eleven logical work packages have been implemented on the
same branch without commits or PRs. Final verification is recorded in
[implementation results](implementation.md). Neutral policy ownership uses
foundation and optional web adapters instead of the originally suggested reverse
security→web dependency. ConfigCapabilities also addresses feature-unification
intent explicitly. The following is the retained original work-package plan.

All changes below were initially **proposed**. Approval of [ADR-001](adr-001-modular-framework.md)
precedes implementation. Each package is a separately reviewable PR; every PR
must leave the existing standard path functional and fail closed. Use temporary
compatibility feature forwarding internally until the facade switches defaults.
Do not ship an intermediate version with silently missing enforcement.

Common Definition of Done: focused source/API review, documented migration,
relevant existing regressions plus new behavior tests passing, fmt/check/tests/
Clippy passing, recorded external consumer normal closure for affected cases,
and no unreviewed changes to ownership or error handling. Security changes need
an independent security-focused reviewer. This plan does not authorize commits,
publishing or changes to the downstream repository during Phase 0.

## 1. ARCH-01b: Generic configuration and explicit schema composition

- **Problem/files:** config `Cargo.toml`, `src/{lib,models,sources,extensions}.rs`,
  config tests, macro `config.rs`, facade `Cargo.toml`, BootUI `Cargo.toml`.
  Aggregate fields, dynamic scheduled schemas and secret paths pull modules
  into generic Properties consumers.
- **Implementation:** introduce additive explicit schema metadata for omitted
  secrets/dynamic maps; route aggregate loading through schema-aware resolution.
  Keep generic load/Properties signatures and error sanitization. Make module
  dependencies optional with integration config features, cfg aggregate fields
  and validators, and a standard bundle. Explicitly forward old integration
  set from facade and BootUI so current callers keep compiling during staging.
  Keep generic default empty. Reject unavailable reserved input sections safely
  and validate known framework keys while permitting application namespaces.
- **Depends on:** ADR approval and verified #78 (already present).
- **API/risk:** direct config aggregate users add module features; keep trait
  default methods and aggregate/extension paths. Highest risk: secret/env schema
  changes or ignored security files; require the entire #75/#78 matrix.
- **Tests:** external config-only `Properties<T>` with file/profile/env/final
  overrides; all #78 cases; omitted secrets; dynamic task names; explicit-source
  failures; unavailable/unknown security sections including null overrides;
  effective config delivered to BootUI.
- **Acceptance/DoD:** config-only normal closure excludes security, scheduler,
  actuator, Axum, JWT, reqwest; existing facade behavior passes via explicit
  forwarding; no duplicated loader or runtime discovery registry.

## 2. ARCH-01c1: HTTP/authorization declaration ownership

- **Problem/files:** web `src/{lib,registration,routing,server,errors}.rs`,
  security `src/{lib,policy,middleware}.rs`, both manifests, facade `lib.rs`,
  build renderer and affected tests. Web's signature and metadata dependencies
  prevent separating enforcement.
- **Implementation:** move policy declaration structs/enums/inventory records
  to web; re-export from security. Add neutral router finish/validation APIs,
  supplied reserved routes, and server callbacks independent of security config.
  Remove web→security before introducing security→web in the same atomic PR.
  Compose enforcement in facade; provide root/facade compatibility wrappers.
  Update generated standard routing paths to selected facade helpers, preserving
  public graph signatures where feasible until macro package 7.
- **Depends on:** 1; internal facade module organization from #82 may land first.
- **API/risk:** direct web integrated builders migrate; metadata inventory type
  identity and policy precedence are security critical. Web-only finish rejects
  effective protected policy even with no enforcement installed.
- **Tests:** direct minimal web consumer; policy matching/HEAD/path precedence,
  explicit public override, roles/scopes, disabled security, handwritten routes,
  duplicate metadata, wrong/missing credentials, generated and legacy adapters.
- **Acceptance/DoD:** no web→security normal edge; no metadata is cfg-elided;
  existing standard auth tests pass; no security→web→security cycle.

## 3. ARCH-01c2: Actuator integration boundary

- **Problem/files:** web routing/server/errors, actuator routing/registry,
  facade composition, build renderer, web/actuator tests. Web always constructs
  actuator router and reserves actuator paths.
- **Implementation:** construct operational router in selected facade adapter,
  pass its reserved method/path pairs to neutral web validation, merge before
  final middleware. Add optional web-independent conversion of registry errors
  at the integration owner; remove web→actuator edge. Keep direct actuator API.
- **Depends on:** 2 (neutral routing primitives).
- **API/risk:** facade wrappers retain selected integrated signatures; direct
  web integrated builders migrate. Risk of endpoint collisions, unintended
  exposure or readiness fallback; no default endpoint enabling.
- **Tests:** no endpoints when absent/empty, all aliases/collisions, registry
  per app, unknown readiness names (#73), own bearer guard and application
  security together, timeouts and health state lifetime.
- **Acceptance/DoD:** web-only normal closure excludes actuator; fallible standard
  startup and legacy DOWN behavior preserved; no cross-registry leakage.

## 4. ARCH-01f: Optional JWT and remote JWKS

- **Problem/files:** security manifest and `src/{config,authentication,middleware,
  jwt,jwks}.rs`, config schema/validation adapters and tests. Security always
  compiles client/crypto stacks and allocates remote cache state.
- **Implementation:** explicit `jwt` and `jwks` features, optional jsonwebtoken
  and reqwest, remote cache only in selected implementation, local verification
  usable without remote client. Remove reqwest::Url dependency from nonremote
  config path using an appropriate independently selected parser/adapter.
  Keep config intent inspectable so unavailable capabilities reject startup.
  Facade temporarily explicitly enables old set until package 6 changes it.
- **Depends on:** 1–2; independent of actuator internals after neutral routing.
- **API/risk:** JWT methods/variants conditional; missing capabilities must never
  route to static bearer fallback. Keep verifier and cache security policy.
- **Tests:** Basic/static bearer no-JWT closure; local JWT no-reqwest closure;
  missing feature config from every source; signature/issuer/audience/expiry,
  algorithms, discovery, time bounds, kid collisions, expiry/rotation/refresh.
- **Acceptance/DoD:** jwt-only excludes reqwest; security-only excludes
  jsonwebtoken and reqwest; negative auth tests pass in each selected build.

## 5. ARCH-01e: Separate lifecycle ownership from selected runners

- **Problem/files:** facade startup/error/private code (currently `src/lib.rs`),
  core resource traits/tests, web callbacks, scheduler handles, BootUI extension
  invocation. HTTP runner unconditionally starts scheduler and knows every config.
- **Implementation:** extract shared initialization/failure/shutdown sequencing
  while keeping existing signatures. Statically include scheduler start/drain
  only when selected; core remains executor-neutral. Bind HTTP only in HTTP
  runner. Keep post-bind extension effective-config handoff and selected logging.
  No new worker execution/supervisor API; #49 owns that scope.
- **Depends on:** 1–3; uses 4 for selected security validation.
- **API/risk:** preserve AsyncResource/Set, shutdown reason/error kind and callback
  behavior. Risks include early Arc drop, double cleanup and skipped rollback.
- **Tests:** fault injection at config/provider/router/hook/scheduler/bind phases,
  HTTP drain/cancellation order, reverse cleanup, timeout and error precedence;
  scheduler disabled/absent; framework-neutral hooks; existing async example.
- **Acceptance/DoD:** one lifecycle owner, no core Axum/Tokio dependency, no
  resource ownership change, standard runtime and startup failure regressions pass.

## 6. ARCH-01d1: Facade features and compatibility wiring

- **Problem/files:** facade/config/security/BootUI manifests, facade exports,
  prelude/error/startup helpers, root workspace dependency forwarding.
- **Implementation:** approved default and feature graph with `dep:` optional
  dependencies, disabled subcrate defaults and explicit adapters. Gate re-exports
  and payload error variants; preserve selected standard names/categories.
  BootUI activation must retain inventory link/start exactly once. Component and
  config macros remain core-compatible. Do not expose a partial unsafe feature
  combination; include the capability token adapters required by package 7.
- **Depends on:** 1–5. Package 7's generator support must land before flipping
  public defaults, or package 6 retains compatibility defaults until 7 lands.
- **API/risk:** empty defaults, conditional fields/error variants and HTTP Result
  alias are intentional pre-1.0 changes. Add an explicit standard migration.
  Cargo feature unification must be tested with two consumers enabling a union.
- **Tests:** isolated minimal, default, web, security, jwt, jwks, actuator,
  scheduler, standard, BootUI; assert package absence and public imports.
- **Acceptance/DoD:** manifests alone produce approved closures; cfg exports
  never hide a remaining normal dependency; no unsupported feature combinations
  compile into unprotected behavior.

## 7. ARCH-01d2: Feature-safe procedural macros and graph output

- **Problem/files:** macros `src/{main,controller,routes,service,bean,config}.rs`,
  build `src/{renderer,parser,lib}.rs`, facade hidden adapters, compile fixtures.
- **Implementation:** render core graph/resources separately from HTTP items.
  Wrap optional generated tokens with facade-selected macro_rules adapters;
  main's required-web adapter selects standard HTTP path or a clear diagnostic.
  Controller/routes need web but policy records do not need enforcement.
  Keep graph build-script and main expansion aligned; do not use application
  feature cfgs or compiler-host feature guesses to select target APIs.
- **Depends on:** 2–5; adapter API coordinated with 6. Can land ahead of 6 using
  compatibility definitions before the facade default switches.
- **API/risk:** generated method/path compatibility, retained resources on router
  failure, and mismatch between proc-macro and facade versions. Bean providers
  still have no implicit output trait impl; do not replace compile-time DI.
- **Tests:** external service/bean/config without web; empty graph with web;
  main/controller without web diagnostic; authenticated web without security
  startup rejection; graph compile pass/fail and #41/#74 fixtures; renamed
  dependencies if supported (do not claim new alias support without tests).
- **Acceptance/DoD:** every supported generated selection compiles without omitted
  APIs; retained hidden standard paths and failure cleanup work. Flip defaults
  only after packages 6 and 7 are jointly validated.

## 8. ARCH-01g1: CLI-generated applications and migration documentation

- **Problem/files:** CLI `templates.rs`, `cli.rs`, `generator.rs`, tests/generated_projects,
  docs/getting-started, configuration, generated-code, security, examples.
- **Implementation:** web/api/service/monolith templates explicitly choose web;
  capability options, if added, produce tested feature lists. Keep service's
  HTTP example labeled accurately; scheduler/core examples use owned entrypoints.
  Document all conditional imports/config and direct web migration, safe
  endpoint exposure, defaults and additive feature unification.
- **Depends on:** 6–7.
- **API/risk:** generated manifests change intentionally, preserve CLI aliases,
  source layout, relative local and registry dependency modes. Do not generate
  security config without the enforcement feature or imply worker supervision.
- **Tests:** all four generated kinds compile externally, serve representative
  requests and shut down; dev watcher config selection; explicit config from
  foreign cwd; generated protected/actuator selections and offline fixtures.
- **Acceptance/DoD:** generated code depends only on selected capabilities; docs
  distinguish supported workers from #49; existing CLI safety tests pass.

## 9. ARCH-01g2: External consumer and negative security matrix

- **Problem/files:** `scripts/arch103-consumers.py`, dedicated consumer fixtures,
  framework integration tests and downstream compatibility documentation.
- **Implementation:** promote proposed fixture mode to CI acceptance, add request
  and startup/error assertions from [security review](compatibility-security.md).
  Keep each selection in a separate Cargo invocation with its own lock/manifest;
  selected target trees are authoritative. Add expected compile-fail fixtures.
  Do not assume all-features proves dependency isolation.
- **Depends on:** 6–8; basic closure gates can land with each earlier package.
- **API/risk:** exercise public APIs only; don't satisfy absence assertions by
  deleting source usage. Ask for separate authorization before editing actual
  Scafra-test-services; temporary copies can prove compatibility immediately.
- **Tests:** [feature matrix](feature-matrix.md), HTTP-free scheduler registration,
  no-web typed properties, protected route failures, configured missing JWT/JWKS,
  ready-check rejection and resource fault injection. Explicitly run ignored
  loopback smoke in an environment that supports it.
- **Acceptance/DoD:** every approved row passes both behavior and forbidden
  package checks; current baseline comparison uses same fixture/profile/lock
  procedure and no unsupported speed claim.

## 10. ARCH-01g3: CI/MSRV feature selection

- **Problem/files:** `.github/workflows/ci.yml`, consumer scripts/fixtures,
  contributing/release docs. Workspace all-features masks unified features.
- **Implementation:** separate external cases for default/minimal/web/secure web/
  actuator/scheduler/standard/all, targeted runtime negatives and fmt/lint/tests.
  Pin declared MSRV 1.99.1 plus stable; approve any extra targets explicitly.
  Cache by target/toolchain/lock/feature selection while keeping app roots
  isolated. Retain workspace integration checks.
- **Depends on:** 9 and approved toolchain/target policy.
- **API/risk:** additive union may conceal failure if fixtures become workspace
  members; no secrets in diagnostic artifacts/config logs.
- **Tests:** run jobs from a clean consumer target with no inherited features;
  verify cache reuse doesn't substitute an all-features manifest.
- **Acceptance/DoD:** failed absent-dependency assertions fail CI; no sole reliance
  on all-features; contributor instructions reproduce the jobs locally.

## 11. ARCH-01g4: Focused publication-order and registry validation

- **Problem/files:** both package arrays in `.github/workflows/publish.yml`,
  `docs/publishing.md`, small DAG validation tool if needed. Build currently
  follows macros despite being macros' dependency; `package --list` is insufficient.
- **Implementation:** reorder build before macros immediately in a focused PR;
  derive/check all selected local normal/build dependency ordering, including
  new web→declaration ownership direction. Package and verify supported feature
  variants against staged available registry dependencies. Do not rewrite tag,
  secret or snapshot logic incidentally. Align coordinated version constraints.
- **Depends on:** current order repair is independent and can land early;
  final graph/package proof depends on 6–10. Coordinate #16/#33/#42/#39.
- **API/risk:** registry versions must expose the same macro/facade adapter
  contract; optional dependencies must also exist in registry metadata.
- **Tests:** DAG cycle/order check, cargo package with compilation verification
  in registry/staging conditions, external registry consumer (not only paths).
- **Acceptance/DoD:** dependency-first order validated; registry-only selected
  consumers pass, supported MSRV/features documented; actual publishing remains
  a separately authorized release action.

Phase 0 completion means these work packages and measurement artifacts exist.
Issue #103 remains open until implementation, review and final matrix pass.
