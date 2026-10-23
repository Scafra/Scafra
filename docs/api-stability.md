# API stability and compatibility policy

Policy revision 1, proposed for maintainer approval through issue #16.
Audited against `986f1bb` (merged PR #104 / issue #103). The workspace is
currently **0.1.0**, not a released stable 1.0 framework. Approval of this policy
sets the release contract; it does not approve the 1.0 release itself.

## Scope and stability levels

The contract covers Rust source compatibility and documented observable
behavior for the supported packages, feature selections, compiler and target
below. It includes configuration interpretation, routing, error categories,
security enforcement, initialization, rollback and shutdown. It does not
promise a Rust ABI, binary compatibility, serialized memory layouts, exact
compiler diagnostics, private module layout, log wording, performance numbers,
or interoperability with arbitrary versions of third-party crates.

- **Stable candidate:** implemented application contracts listed below, intended
  to receive the 1.x guarantee after explicit 1.0 release approval. Until then
  they follow the 0.x rule below.
- **Experimental:** the explicitly listed interfaces below may change in minor
  releases (0.x: next 0.y). Announce changes and provide migration instructions;
  patches still preserve their supported usage. Experimental does not relax
  security, dependency resolution, or MSRV requirements. Promotion requires
  documentation, consumer tests and maintainer approval in a minor or major release.
- **Internal implementation:** generated plumbing and implementation modules;
  applications must not call these directly. Public visibility and
  `#[doc(hidden)]` do not make changes technically non-breaking. The framework
  must preserve their macro/runtime interoperability for supported resolutions.
- **Future / unsupported:** no implementation or compatibility promise exists.

Unlisted items are **not automatically stable merely because they are `pub`**.
Before 1.0 the release reviewer must classify newly exposed application APIs in
this inventory. Once 1.0 ships, an ordinary documented public item in a supported
crate is stable unless explicitly marked experimental or internal at introduction.
An omission from this inventory cannot retroactively withdraw a shipped promise.
Reclassifying a stable API as experimental/internal requires a major release.

## Source-verified API inventory

Paths below are relative to this repository. Candidate rows include their
public fields, variants, associated items, implementations and feature-gated
re-exports unless a specific exception is stated. The guides define accepted
syntax and behavior, rather than every syntax that happens to parse today.

| Area and classification | Actual contracts and behavior | Source / specification |
| --- | --- | --- |
| Core — stable candidate | `Application::{new,context,state,with_hook,start,shutdown,shutdown_with}`, `ApplicationContext::{new,metadata,discover}`, `LifecycleHook`, `LifecycleState`, `LifecyclePhase`, `LifecycleError`, `LifecycleFailure`, `ScafraError`. Context holds component metadata; it is not a bean lookup container. Ordered startup, rollback and reverse shutdown remain observable contracts. | [lifecycle.rs](../crates/scafra-core/src/lifecycle.rs), [errors.rs](../crates/scafra-core/src/errors.rs) |
| Components — stable candidate | `Bean` is an optional `Send + Sync + 'static` marker, not a composition requirement. `Service`, `BeanPostProcessor`, component kinds/metadata and registration declarations support compile-time composition and lifecycle registration. A repository annotation does not implement persistence. | [registration.rs](../crates/scafra-core/src/registration.rs) |
| Typed graph — stable candidate | `GraphDescriptor`, `GraphNodeDescriptor`, `GraphEdgeDescriptor`, `GraphNodeKind`, `GraphPlan`, `GraphError`, `GraphPhase`, `ProviderFailure`. Deterministic dependency ordering; duplicate/missing/owned-dependency/cycle errors and typed provider failure chains. Generated composition and typed overrides are described below. No runtime service locator. | [graph.rs](../crates/scafra-core/src/graph.rs), [graph guide](guides/dependency-graph.md) |
| Resources — stable candidate | `AsyncResource` with associated `ShutdownError`, `request_shutdown`, async `shutdown`; `AsyncResourceSet` including the unit implementation. Sequential topological initialization, retained owners, reverse cleanup and aggregation of failures. Async consumers require `Arc<T>`; partial work in a failing provider remains its responsibility. | [registration.rs](../crates/scafra-core/src/registration.rs), [resource guide](guides/resource-lifecycle.md) |
| Foundation — stable candidate | `ShutdownPolicy`, `ShutdownReason`, `Phase`, `PhaseMetadata`, logging configuration/initialization and tracing macros, `AuthorizationMode`, `AuthorizationPolicy`, controller/route policy metadata. Shared re-exports retain type identity. Debug/backtrace formatting is not a machine protocol. | [foundation exports](../crates/scafra-foundation/src/lib.rs) |
| Configuration — stable candidate | `ScafraConfig`, `ServerConfig`, `StartupConfig`, `LoggingConfig`, `ConfigLoader`, `Properties<T>`, `Config`, `ConfigProperties`, `ConfigCapabilities`, `ConfigError`, `ValidationError`, `load_yaml`. Defaults → base → profile → environment → explicit override precedence, deterministic source selection, required explicit files, schema-aware environment mapping and validation. Raw domain loading does not reserve framework namespaces; validated framework loading does. Capability masks only narrow compiled capabilities. | [sources.rs](../crates/scafra-config/src/sources.rs), [models.rs](../crates/scafra-config/src/models.rs), [configuration](configuration.md), [modules](modules.md) |
| Feature-dependent configuration — stable candidate | Aggregate fields `security`, `actuator`, `scheduler`, `bootui` appear only with their features; server/logging/startup remain present. `JwtConfig` remains exported with security even without JWT verification, so unavailable JWT intent can be rejected. Field presence does not imply implementation availability. Validation rejects unknown/unavailable framework settings before merge; optional extension payload types do not independently activate runtime modules. | [models.rs](../crates/scafra-config/src/models.rs), [security config](../crates/scafra-security/src/config.rs) |
| HTTP — stable candidate | `ControllerPrefix`, `ControllerRoutes`, `ControllerRegistration`, `RouteMetadata`, `JsonBody<T>` extractor, `AppError`, `WebError`, `ServerError`, root `Result<T>` (web only); Axum 0.8 / Tower 0.5 interoperation via `scafra::web::{axum,tower}`. Public JSON error/status behavior, body limits and configured request timeout are contracts. An upstream incompatible exposed type change needs a Scafra major release. | [web exports](../crates/scafra-web/src/lib.rs), [JSON](../crates/scafra-web/src/json.rs), [errors](../crates/scafra-web/src/errors.rs) |
| Router composition — stable candidate | `RouterOptions`, `build_router_with_options`, `finish_router_with_options`, `validate_routes_with_options`, `operational_router`, `finish_managed_router`, `guard_axum_router`, `FinalizedRouter::{nest,into_router}`, existing feature-gated builders and serving adapters. Raw/global serving applies inherited security to arbitrary routes; managed finalization preserves generated route-local policies and requires explicit guards for handwritten routes. Finalized serving avoids duplicate finalization. Metadata is required for validation, not URI-based policy reconstruction. | [routing.rs](../crates/scafra-web/src/routing.rs), [server.rs](../crates/scafra-web/src/server.rs), [composition guide](modules.md#configuration-and-fail-closed-security) |
| Security — stable candidate | `SecurityConfig`, `BasicAuthConfig`, `JwtConfig`, policy declarations, public security layering/context APIs. Disabled by default at runtime; explicit protected routes reject unavailable/disabled enforcement. Role groups combine AND, names within a group OR; scopes AND. JWT signature, algorithm, expiration and configured issuer/audience validation precede claims. JWKS cache/rotation and fail-closed rules follow the security guide. No fallback from invalid JWT to Basic/static bearer. | [security exports](../crates/scafra-security/src/lib.rs), [security guide](security.md) |
| Startup — stable candidate | `#[scafra::main]` is an HTTP entrypoint requiring `web`; `run`, `router_options`, `run_with_router`, `run_with_resources`, `run_with_finalized_router`, `run_with_finalized_resources`, `StartupError` and `kind()`. Config validation precedes resource creation/listening. Graceful serving, `shutdown_channel`, shutdown handles/futures and `ServerOutcome` are supported. Resource ownership survives serving and startup failure. | [startup.rs](../crates/scafra/src/startup.rs), [main expansion](../crates/scafra-macros/src/main.rs) |
| Scheduler — stable candidate | `SchedulerConfig`, task config/registration, `register_scheduled_task!`, `Scheduler::{try_start,start,metrics,shutdown_handle,stop,shutdown}`, metrics/errors/handle. Fixed-delay non-overlap on Tokio's blocking pool; shutdown joins callbacks. A non-returning callback can block shutdown indefinitely. | [scheduler](../crates/scafra-scheduler/src/lib.rs), [guide](guides/scheduler.md) |
| Operations — stable candidate | Actuator config, endpoint selection, health registry/errors/check future, `register_health_check!`, router APIs, readiness/liveness/health/info/metrics behavior. Endpoint selection and independent bearer guard remain explicit. Unknown configured readiness checks fail; they cannot silently report healthy. | [actuator exports](../crates/scafra-actuator/src/lib.rs), [module reference](modules.md) |
| Optional extensions — experimental | `OptionalExtensionRegistration`, `ConfiguredExtensionRegistration`; BootUI `start`, `start_with_config`, `layer`, `try_layer_with_config`, `try_layer_with_config_and_profile`, `BootUiConfig`, `BootUiPolicyError`, dashboard HTML/JSON schema. Explicit selection is retained; peer/local-only protection and allow-listed snapshots remain mandatory even while APIs evolve. | [BootUI](../crates/scafra-bootui/src/lib.rs), [extension registration](../crates/scafra-config/src/extensions.rs) |
| Build tooling — stable candidate / experimental | `discover`, `discover_graph` and documented output inclusion/composition are candidates. `BuildContext`, `BuildExtension`, `BuildOutput`, pipeline/error types, `run_extensions`, `discover_with_extensions`, `discover_graph_with_extensions`, `render_application_graph` are experimental custom pipeline APIs. | [build exports](../crates/scafra-build/src/lib.rs), [extensions](../crates/scafra-build/src/extensions.rs) |
| CLI — stable candidate / experimental | `new` and its four kinds, explicit `--features`, `check`, `dev` command/exit behavior and compilable generated manifests are candidates. Exact template text, console wording and debug auto-reload supervisor behavior are experimental. `service` currently generates HTTP, not a supervised worker. CLI is a binary, not a Rust library API. | [CLI](../crates/scafra-cli/src/cli.rs), [generated code](generated-code.md) |
| Internal implementation | `__private`, `__scafra_if_*`, `__scafra_require_web!`, generated route handlers/registration helpers, raw inventory access, parser/renderer modules and intermediate build models. Ordinary public registration records/traits above remain candidates; reaching inventory through a hidden re-export is internal. | [feature adapters](../crates/scafra/src/feature_macros.rs), [private facade](../crates/scafra/src/private.rs), [renderer](../crates/scafra-build/src/renderer.rs) |
| Future / unsupported | Supervised HTTP-free worker entrypoint (#49), runtime reflection/service lookup, automatic database repository behavior, PostgreSQL integration, browser sessions and static inline/file JWKS. Scheduler-only/core usage works with an application-owned runner; it is not the future worker runner. | [architecture](architecture.md), [security limits](security.md) |

### Procedural macro and generated application contracts

The supported entry point for macros is the facade, with its dependency named
`scafra`: expansions use absolute `::scafra` paths. Renaming that dependency
without supplying this name is not supported. See [graph declarations](guides/dependency-graph.md).

| Macro | Candidate supported expansion/behavior |
| --- | --- |
| `#[service]`, `#[component]`, `#[repository]` | Non-generic structs; typed public `new` constructor; `Service` implementation; unit struct `Default` when not already derived; metadata registration. These three currently share the service expansion, including its registration kind. Do not infer a persistence adapter or a distinct generated trait for repository/component. |
| `#[bean]` | Non-generic free provider functions, concrete owned outputs, optional two-parameter `Result<T,E>`, sync/async graph construction. Public application-owned struct outputs/newtypes, typed dependency arguments. Preserves function; registers metadata; does **not** synthesize `Bean` or `Default` for outputs. |
| `#[controller]` | Typed `new`, unit `Default`, `ControllerPrefix::{PREFIX,AUTHORIZATION_POLICY}`, registration and policy declarations. Requires web. |
| `#[routes]` | `ControllerRoutes` implementation including route/authorization metadata and instance registration, optional default registration; method aliases, get/post/put/delete and public/authenticated/roles/scopes helper declarations. Generated `MethodRouter` guards preserve selected handler policy across nest/merge/HEAD. Standalone get/post/put/delete macros are no-op attributes; they only declare routes inside `#[routes]`. |
| `#[scafra::main]` | Discovers `src/main`, emits modules and typed graph, initializes Tokio, validates configuration, composes async resources, finalizes and runs HTTP, performs cleanup and bounded failure logging. Entry function syntax and documented discovery conventions are supported; exact token text is not. |
| `#[derive(Config)]` | Implements `Config<Error = ValidationError>` and `ConfigProperties`, emits inherent `CONFIG_PREFIX`; `#[config(prefix = "...")]`, named `#[config(required)]` String/Option validation. Requires the documented Serde/default bounds for loading. |
| `#[post_processor]`, `#[logger]`, `startup!` | Post-processor Default/registration and application-provided trait implementation; function/method tracing spans; foundation startup logging macro. Preserve supported inputs and generated public obligations, not diagnostic wording. |

Generated `__scafra_graph` is **not wholly internal**: `descriptor()`, `Graph`,
`Resources: AsyncResourceSet`, `Overrides: Default`, `compose_with`, `compose`,
`compose_async_with`, `compose_async`, `into_resources`, and documented router
builders are application-facing candidates. Sync compose and resource-dropping
convenience builders are only emitted for graphs without async resources.
`build_router_with_options_and_resources` and
`build_finalized_router_with_options_and_resources` retain ownership on error.
Legacy integrated builders require actuator + security. `Graph` leaf fields and
`Overrides` fields are typed `node_N` slots determined by the application's
current graph. Supported behavior includes overriding a provider with its typed
value; adding/reordering application providers can change slot numbers. There
is no stable index across different application graphs. For unchanged supported
input, an expansion change that breaks these APIs, trait implementations,
constructor signatures, bounds, hygiene or runtime behavior is a breaking change.

## Release lines and compatibility matrix

For existing **0.y.z (y > 0)** releases, patches preserve the supported APIs,
features, behavior and MSRV of that 0.y line; compatible additions may also land
in a patch during initial development. Breaking changes require the next 0.y
with rationale and migration notes. Thus `^0.1` accepts 0.1 patches, not 0.2.
The modular change landed during development of current 0.1.0; this policy does
not assert compatibility with previous snapshots merely sharing the base number.
0.0 releases would have no compatibility line; none is the current workspace.
The initial **1.0.0** freezes approved stable candidates at the release commit;
experimental/internal exceptions must be recorded there before publication.

After 1.0, patch = compatible corrective change, minor = compatible functionality,
major = intentional incompatible contract. Deprecation alone does not permit
removal within 1.x. The following is deliberately more conservative than
Cargo's general guidance for possibly-breaking additions. See the official
[Cargo compatibility guidance](https://doc.rust-lang.org/cargo/reference/semver.html)
for the Rust language hazards underlying this matrix.

| Change to supported contract | Patch 1.x.z | Minor 1.y.0 | Major N.0.0 |
| --- | --- | --- | --- |
| Compatible bug fix; private implementation/dependency fix | Yes | Yes | Yes |
| New independent public item / capability | No, except minimal compatible repair | Yes after collision/inference review | Yes |
| Remove/rename/move item or re-export without preserving old path | No | No | Yes, migration required |
| Signature, lifetime, associated type, generic-bound tightening, Send/Sync or ownership change that breaks valid callers/implementors | No | No | Yes |
| Required trait method/associated item, supertrait, loss of dyn compatibility; change associated type or RPIT capture obligations | No | No | Yes |
| Default trait method, inherent method, blanket/trait implementation, generalized signature | Only compatible repair | Only if downstream method resolution, coherence and inference remain compatible | Yes if incompatible |
| Variant added to existing exhaustive enum (including errors), or fields added to exhaustive variants | No | No | Yes |
| Field added to publicly constructible exhaustive struct, tuple arity changed, fields removed/renamed/retargeted | No | No | Yes |
| Variant/field addition behind an **already shipped** applicable `non_exhaustive` boundary | Only compatible repair | Yes if construction/patterns/behavior remain compatible | Yes |
| Add `non_exhaustive` to an existing exhaustive type | No | No | Yes |
| Macro supported syntax/expansion or generated public API breaks | No | No | Yes |
| New optional config key with compatible Rust representation and old behavior retained | Only corrective repair | Yes | Yes |
| Remove/rename key, change interpretation/precedence, newly require config, change documented defaults or errors incompatibly | No | No | Yes |
| Remove/rename feature; remove its capability/dependency; add mandatory integration to existing feature/bundle | No | No | Yes |
| Any change to facade default feature set (currently empty) | No | No | Yes |
| Existing feature gets optional compatible functionality, with no new default behavior/dependency burden or API loss under unification | Only corrective repair | Yes after isolated/union checks | Yes |
| Exposed upstream dependency type gets incompatible identity/version, or newly forces incompatible dependency/MSRV | No | No | Yes |
| MSRV increase | No | No | Yes |
| Remove officially tested target or supported behavior | No | No | Yes |
| Experimental API incompatible change, disclosed in release notes | No | Yes with migration | Yes |

A public addition is not automatically compatible: glob imports can become
ambiguous; an inherent/default trait method can shadow an extension method;
new blanket implementations can overlap downstream implementations; changed
bounds or impl availability can alter inference, method resolution and trait
coherence. Review concrete downstream usage; when an addition breaks supported
usage, use a major release or a different API name. Existing exhaustive error
enums and all-public config structs in Scafra are real constraints: Serde
`default` does not make a new Rust field source-compatible. An additive feature
must also build alongside every existing feature it can be unified with.

Security corrections restoring documented rejection/redaction may ship as
patches even if applications relied on a vulnerability or previously accepted
invalid input. Record the impact, regression and migration in release notes;
do not quietly change a valid supported policy under the label “bug fix”. No
release may turn protection into anonymous access to preserve availability.

## Supported crates and coordinated versions

All twelve crates form one coordinated distribution and inherit the workspace
version and Rust version. Publish a complete package set from one commit,
including crates whose implementation did not change. There is no independent
minor/patch cadence or promise for arbitrary package combinations.

| Package | Direct usage / supported surface at 1.0 approval |
| --- | --- |
| `scafra` | Preferred application dependency: candidates at root, `prelude`, `core`, `config`, `foundation`, feature-selected `web`, `actuator`, `bootui`; the last remains experimental. |
| `scafra-core` | Supported direct HTTP-free dependency for listed core contracts. |
| `scafra-foundation` | Supported direct dependency for shared policies, logging and metadata; one type identity across re-exports. |
| `scafra-config` | Supported direct typed loader/properties dependency; select its own integration features. |
| `scafra-web` | Supported direct HTTP/router/server dependency with its own feature set; macros still require facade paths. |
| `scafra-security` | Supported direct root configuration, policy and middleware API, with own jwt/jwks opt-ins. Private submodules are internal. |
| `scafra-actuator` | Supported direct health/operational integration and root API. |
| `scafra-scheduler` | Supported direct scheduling and drain APIs; caller supplies runtime without facade HTTP. |
| `scafra-bootui` | Supported selectable package, explicitly experimental dashboard/extension APIs; no stable UI/JSON schema promise. |
| `scafra-macros` | Published implementation package; use facade macro re-exports. Direct imports do not create a standalone runtime compatibility guarantee. |
| `scafra-build` | Supported build dependency for discovery; custom extension/render APIs experimental. Match the facade release when generating code. |
| `scafra-cli` | Supported installed tool and documented commands, paired with the target runtime release for generation; not a direct Rust library dependency. |

Recommended/tested pairing: resolve facade, macros, build, core, web and
foundation (and selected companion crates) to the **same full release version**.
Applications with direct companions should update them together, inspect
`cargo tree -d`, and retain Cargo.lock. The manifests currently specify ordinary
compatible ranges (`version = "0.1.0"` means `^0.1.0`), not exact pins. Cargo can
therefore select newer compatible packages, and resolver v2 can build separate
host/build feature sets. The release obligation includes preserving generated
plumbing across package versions admitted by those ranges in the same
compatibility line, including newly released macros resolving against older
permitted runtime packages. If a new macro needs a newly added helper, raise
its relevant runtime dependency lower bound through its build/core dependency
chain or preserve/fallback to the old contract, then verify a mixed-resolution
consumer. This lower-bound technique only works where an actual dependency
edge constrains the helper-owning package: macros have no direct web dependency,
so a new web helper cannot be required merely by raising macros/build/core
versions. Preserve the old web expansion contract or change the coordinated
compatibility line unless a verified resolver constraint supplies the needed
web minimum. A facade macro dependency lower bound alone does not constrain the
facade version seen by a downstream expansion; inspect the full resolution.

No exact pins are added here. Exact matching all internal packages would force
lockstep patch upgrades and can create unsatisfiable same-line version
constraints between downstream dependencies; permitting duplicate incompatible
versions can instead split shared Rust type/inventory identity. Pinning is not
a substitute for compatibility. Before any future requirement change, verify
clean external resolution and mixed lower-bound/latest consumers. If the
hidden contract cannot be preserved compatibly, bump the coordinated major
(0.x: minor), update bounds and publish order together, and rebuild generated
applications against the new set. Do not reuse emitted graph artifacts from
another release; regenerate from application source.

## Cargo feature contract

Decision proposed for 1.0: **preserve empty defaults**. This permits actual
HTTP-free composition/configuration and avoids compulsory authentication,
remote clients, operations or scheduling. CLI starters explicitly select web,
so this does not prevent the intended quick-start experience. The feature table
in [modules](modules.md#cargo-feature-reference) defines capability meanings.
The checked manifest contract is recorded below; dependency forwarding is part
of the review boundary, not just the feature names.

<!-- compatibility-contract -->
```toml
msrv = "1.88"
verified_target = "x86_64-unknown-linux-gnu"
packages = ["scafra", "scafra-core", "scafra-foundation", "scafra-config", "scafra-web", "scafra-security", "scafra-actuator", "scafra-scheduler", "scafra-bootui", "scafra-macros", "scafra-build", "scafra-cli"]

[features]
default = []
web = ["dep:scafra-web", "dep:tokio", "dep:axum"]
security = ["web", "dep:scafra-security", "scafra-config/security", "scafra-web/security"]
jwt = ["security", "scafra-security/jwt", "scafra-config/jwt", "scafra-web/jwt"]
jwks = ["jwt", "scafra-security/jwks", "scafra-config/jwks", "scafra-web/jwks"]
actuator = ["web", "dep:scafra-actuator", "scafra-config/actuator", "scafra-web/actuator"]
scheduler = ["dep:scafra-scheduler", "scafra-config/scheduler", "dep:tokio"]
bootui = ["security", "dep:scafra-bootui", "scafra-config/bootui"]
standard = ["web", "security", "jwt", "jwks", "actuator", "scheduler"]
```

`standard` freezes this bundle for 1.x; new modules require explicit selection
or a new named bundle. It is a capability migration aid, not automatic source
compatibility for every pre-modular application. BootUI stays separate.
`security` supplies enforcement code but does not set `security.enabled` or
credentials. `jwt` includes security, `jwks` includes JWT, BootUI includes
security, actuator includes web, scheduler need not include web.

All unions of named capabilities are intended to compose; CI verifies
representative isolated minimal/default/config/core/web/security/JWT/JWKS/
actuator/scheduler/BootUI/standard/all plus nested/inventory consumers, not
all 256 selections. Unsupported macro use (HTTP macros without web) diagnoses
missing capability. Lower-level config/web/security features follow their own
manifest names; facade forwarding does not retroactively select facade features
when a companion is enabled directly.

Cargo features are additive and unified per resolved package/build context.
`default-features = false` on one edge cannot disable another edge's features.
See [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html).
Adding capability to an existing feature is reviewed for normal dependency
closure, public fields/variants, validation, MSRV and runtime behavior under
unification. Required dependency changes that violate the frozen meaning need
a major release. Ordinary compatible private dependency fixes are allowed.

### Security compatibility invariants

- Missing/disabled security cannot make explicitly protected metadata public.
  Router construction fails; route-local guards deny when effective context is
  missing. Unavailable security/JWT/JWKS configuration must fail closed.
- Validated framework loading checks unavailable/unknown namespaces even when
  disabled, in files/profiles/environment/overrides before merging. Generic
  domain schemas remain independent. The runner narrows capabilities to its own
  facade selections despite dependency feature union.
- Generated handler roles/scopes and public override precedence survive nest,
  merge, actual method/path selection and GET→HEAD. Managed handwritten routes
  require explicit guards before merging; arbitrary raw routers use global
  APIs. Legacy global layers may conservatively deny nested public overrides.
- Authentication failure never becomes anonymous fallback; preserve denial
  categories and secret redaction. Actuator has independent access controls;
  BootUI peer/bind restrictions and snapshot allow-list remain enforced.

## MSRV and targets

The declared minimum is **Rust/Cargo 1.88**, verified with **1.88.0**. It covers
published library APIs under supported features, procedural macro/build host
code, and CLI consumers on the tested target. At least one dependency resolution
satisfying the declared ranges must work on it, following [Cargo dependency support expectations](https://doc.rust-lang.org/cargo/reference/rust-version.html#support-expectations).
It does not mean every future
version allowed by broad third-party ranges supports 1.88. A library's workspace
lock is not imposed on downstream applications. For a clean consumer, resolve
on 1.88, retain its lock, and use compatible dependency versions if a newer
transitive package raises its MSRV; report regressions with the resolution.
Release review must check clean and locked consumers, adjust lower bounds or
compatible constraints as necessary, and never use `--ignore-rust-version` as
proof. Do not force unnecessary upper bounds/exact pins on the whole ecosystem.

The Scafra source syntax minimum, declared dependency policy minimum, a
particular lockfile's minimum, and release-CI-verified minimum are different.
Historical downresolved 1.82 experiments are not a 1.82 support claim. CI's
existing 1.88 consumer matrix regenerates locks offline using cached registry
metadata, seeded from the workspace; this is useful independent resolution,
but not proof of every latest online resolution. #33 owns continued compiler/
target verification; its issue's original 1.82 text predates #103.

Chosen policy: preserve 1.88 throughout 1.x; an increase requires a **major**
release (0.x: next minor), rationale, dependency evidence, migration notes and
updated CI. This is stricter than Cargo's conventional minor MSRV policy.
Development-only tools, Clippy/rustfmt and tests use current stable unless an
MSRV check explicitly includes them; the consumer promise includes build
scripts/macros required by applications. A tooling-only exception must name
its compiler, scope and reason in release notes without raising library MSRV.

| Support term | Scope and obligation |
| --- | --- |
| Officially tested | Linux x86_64 GNU, `x86_64-unknown-linux-gnu`: Ubuntu CI, stable workspace checks and Rust 1.88 isolated consumers/runtime. This is the sole current verified target. |
| Expected to work | Native macOS/Windows with compatible Rust/dependencies; source has non-Unix signal and platform filesystem branches, but no CI evidence here. No release support guarantee or equivalent SIGTERM claim. |
| Community supported | Other targets with downstream reports/patches; no currently designated additional verified triple. Promotion requires reproducible build/runtime CI and maintainer acceptance under #33. |
| Unsupported | wasm/browser, bare-metal/no_std and other environments lacking required std/network/runtime facilities; no framework runtime guarantee. |

No universal OS/runtime promise follows from `cfg` branches compiling in theory.
Target additions require tests, removals require major releases once officially
supported. Security invariants are never waived on community targets.

## Verification and release review

Run `python3 scripts/check-api-policy.py` and its focused drift tests
`python3 -m unittest discover -s scripts -p 'test_api_policy.py'`.
The checker parses TOML, not Rust text, and compares this contract with actual
workspace members, facade defaults/forwarding, package MSRV/version inheritance
and internal dependency versions. Consumer CI passes its selected `--rust`
value to reject a numeric consumer compiler that differs from the documented
MSRV. Removing MSRV jobs entirely still requires workflow review. No new Cargo
or Python dependency is required (Python 3.11+ already used by modular scripts).

| Risk | Executable evidence / remaining review |
| --- | --- |
| Item/signature/trait/re-export removal | Existing public API integration tests and representative graph/consumer compilation; maintainer diff of public signatures against prior release, for both minimal and enabled APIs. Tests do not enumerate every public item. |
| Macro generated API | Macro UI compile-fail/pass tests, build graph fixtures (overrides, ownership, async rollback) and real facade macro external consumers. Preserve supported expansion, not token snapshots alone. |
| Features/defaults | Policy checker, isolated consumer forbidden-normal-package assertions and negative feature/config suite; all-features workspace alone cannot prove isolation. |
| MSRV | Existing Rust 1.88 CI consumer matrix plus locked workspace check locally/release review; clean published consumer resolution evidence required for RC. |
| Packages/CLI | Publish DAG checker, staged archive verification and two registry-source macro consumers; CLI generated-project tests. #15/#42 additionally require actual published packages without path overrides and all templates. |
| Security/behavior | Security/JWT/JWKS and nested-route regressions, negative feature-union configuration tests, live denial/readiness/SIGTERM/resource cleanup consumers. Review behavior and migration beyond compilation. |

A public API diff tool would improve removed-item/signature coverage. No new
CI tool is installed here: `cargo-public-api` uses rustdoc JSON and a compatible
nightly/tool version, and a semver checker needs a real published baseline plus
feature-specific runs and exception review. Before 1.0, #39/#42 must select a
pinned maintained tool/baseline or record the manual public-API diff artifact,
including direct crates and macros. Tool upgrades require compiler compatibility
and output/noise review. A green diff never proves behavior or macro compatibility.

Minimum evidence before any breaking release: all seven workspace commands in
[the review report](architecture/issue-16-review.md), the policy checker/tests,
MSRV consumers for every CI case, negative suite, publish DAG and staged package
check; generated-project compile/runtime tests; a representative upgrade from
the previous supported line and real RC/published artifact smoke tests (#15/#42).
Attach versions, target, features, locks and exact commit. Require migration
examples, release notes and explicit maintainer decision using the
[compatibility checklist](contributing.md#compatibility-review-checklist).
Stable/RC release approval also requires security advisory triage (#43), measured
benchmark evidence (#9), MSRV/target evidence (#33) and the #39 go/no-go checklist.
A local staged registry is not a crates.io propagation test.

Alpha and snapshot releases are exact-version experiments with no upgrade or
maintenance guarantee; beta signals API review, not stability. RCs freeze the
proposed release contract for validation, but a required fix may change it in a
new RC with notes/retests. Cargo stable requirements do not select prereleases;
use explicit prerelease requirements and preferably exact versions across the
whole coordinated set. None of these labels is a stable release. No automatic
LTS/backport window is promised; #43 defines security ownership/maintenance.
Stable releases require all gates and release/migration notes; major releases
must explain every intentional break. See [publishing](publishing.md) for order
and mechanics and [upgrading](upgrading.md) for application steps.
