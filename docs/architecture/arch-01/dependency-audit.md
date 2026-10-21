# Source and dependency audit

> Historical Phase 0 snapshot/planning evidence. The [implemented contract](../../modules.md)
> and [implementation results](implementation.md) supersede proposed/current-state claims below.

Scope: all twelve workspace crates at `e041fc27c46e521e848ecbd3895d85b876c9b831`.
The working tree was clean before Phase 0. Branch:
`103-arch-01-make-scafra-framework-modules-independently-opt-in-before-100`.
See [exact normal/build/dev inventory](cargo-inventory.md) and
[baseline procedure/results](baseline.md). No runtime code changed.

## Crate responsibilities and intended boundaries

“Independent” below distinguishes today's selected-package compilation from
dependency isolation. All twelve `cargo check -p NAME --no-default-features
--locked --offline` selections pass. These use the workspace lock and a shared
target; they do not prove crates.io packaging. External fixtures separately
check facade, core, config and generated graph consumption outside the workspace.
Every current runtime crate can be consumed through its package API, but several
bring unrelated capabilities. Nothing currently makes a facade dependency optional.

| Crate | Source responsibilities and important external deps | Future selection / unavoidable edges |
| --- | --- | --- |
| `scafra` | `src/lib.rs`: exports, error taxonomy, config/startup, HTTP, scheduler, extensions, supervision and banner; Axum, Tokio, inventory, tracing, thiserror | Unified facade; mandatory core/foundation/config/macros; integrations optional. Standard HTTP entrypoints necessarily use Tokio/Axum when selected |
| `scafra-core` | `lifecycle.rs`, `graph.rs`, `registration.rs`: synchronous hooks, graph descriptors/plans, provider errors, async resource contracts; inventory, thiserror | Mandatory for typed composition; no Axum/Tokio normal edge. Foundation owns phase/shutdown metadata used in signatures |
| `scafra-foundation` | `logging.rs`, `phase.rs`, `shutdown.rs`, `backtrace.rs`: shared policy, logging initialization, diagnostic capture; Serde, tracing/subscriber, thiserror | Shared base; logging subscriber is real current cost even for core-only. Removing it is a separate optimization, not assumed here |
| `scafra-config` | `sources.rs`, `merge.rs`, `models.rs`, `extensions.rs`: generic load, aggregate models/validation, configured extensions; Serde/YAML, inventory, thiserror | Generic mandatory facade capability; integration model edges optional; preserve single loader. Source-specific hooks currently prevent a manifest-only fix |
| `scafra-web` | `registration.rs`, `routing.rs`, `server.rs`, `json.rs`, `errors.rs`: route inventory, limits/tracing, authorization validation, actuator merging, serving and drain; Axum, Tokio, Tower/HTTP | Optional HTTP capability. Axum and HTTP-related runtime are justified; security and actuator edges must be removed |
| `scafra-security` | `config.rs`, `policy.rs`, `authentication.rs`, `middleware.rs`, `jwt.rs`, `jwks.rs`: Basic/static bearer, roles/scopes, JWT, discovery/cache; Axum, base64, jsonwebtoken, reqwest, Serde/JSON, Tokio | Optional enforcement; JWT and remote fetch separately optional. Move declarations to web; security then depends on web, never vice versa |
| `scafra-actuator` | `config.rs`, `registry.rs`, `health.rs`, `routing.rs`, `security.rs`, `handlers.rs`: endpoints, application-owned health registry, legacy inventory, own token guard; Axum, Tokio, Serde, tracing | Optional HTTP operations. Currently no direct Scafra dependency; need not depend on application security for its own bearer guard |
| `scafra-scheduler` | `src/lib.rs`: config/registration validation, interval tasks, metrics, stop/drain handles; Tokio, Serde, inventory, tracing, thiserror | Optional; direct package already has no web/security dependency. Tokio workspace feature inheritance currently includes networking; trim separately if warranted |
| `scafra-bootui` | `lib.rs`, `binding.rs`, `snapshot.rs`, `dashboard.rs`: configured extension, bind policy, sanitized operations snapshots, Axum listener/security layer | Optional explicitly linked dependency today. Depends on config/core/security; keeping data-only BootUiConfig in config avoids config→BootUI→config cycle |
| `scafra-macros` | `main.rs`, `controller.rs`, `routes.rs`, `service.rs`, `bean.rs`, `config.rs`: proc-macro expansion and typed graph generation; syn/quote/proc-macro2 | Mandatory ergonomic facade compile-time dependency; no target runtime execution. Its dependency on build is a **normal host dependency**, not Cargo build-dependencies |
| `scafra-build` | `discovery.rs`, `parser.rs`, `graph.rs`, `renderer.rs`, `extensions.rs`: source discovery, provider resolution, ordered typed graph generation, transactional build outputs; syn/proc-macro2 | Build tooling; optional direct build-script usage, transitive host dependency of macros. Core/foundation needed for graph kind/phase contracts |
| `scafra-cli` | `templates.rs`, `generator.rs`, `commands.rs`, `filesystem.rs`, `cli.rs`: generation, config-aware dev watcher, process handling and safe filesystem writes; Clap/anyhow, target rustix/windows-sys | Development executable only; never a facade runtime dependency. Foundation provides logging policy; no need for runtime module closure |

## Configuration: coupling beyond manifests

- [`models.rs`](../../../crates/scafra-config/src/models.rs) embeds security,
  scheduler and actuator types in `ScafraConfig`. Its validation calls all three
  integration validators and checks BootUI/server settings. Scheduler errors are
  translated to safe field messages; security validation also translates static
  reasons. This behavior must move with composition, not disappear under cfg.
- [`sources.rs`](../../../crates/scafra-config/src/sources.rs) serializes typed
  defaults into an environment schema, wraps property prefixes before merging,
  and resolves environment paths against that schema. `omitted_secret_fields`
  hard-codes `security.jwt.secret`, `security.basic.password`, and
  `actuator.security.bearer_token`. `scheduled_task_schema` directly constructs
  `scafra_scheduler::ScheduledTaskConfig` for dynamic task names. Both need
  explicit schema adapters; removing imports alone breaks environment behavior.
- [`extensions.rs`](../../../crates/scafra-config/src/extensions.rs) exposes
  `ConfiguredExtensionRegistration { start: fn(&ScafraConfig, &str) }`.
  BootUI consumes the effective config/profile, and startup sorts registrations
  by name after HTTP bind. Moving ScafraConfig to the facade would create a
  BootUI→facade→BootUI cycle. Retaining the aggregate in config with optional
  model features is simpler; the public registration contract can remain.
- Precedence is defaults → base yaml/yml/properties → corresponding profile
  files → sorted environment → explicit overrides. Profile selection has its
  own explicit/profile environment/base priority. Explicit sources must exist;
  implicit conventional files may be absent. Merge structure errors remain
  fatal. `load` decodes; validated methods additionally validate.
- Serde models generally allow unknown file keys. Unknown environment paths are
  rejected against the typed schema. Cfg-removing fields while relying on Serde
  defaults would silently ignore file security settings. Strict reserved-section
  rejection is a new intentional behavior, not a current guarantee.

## HTTP, security and actuator

[`routing.rs`](../../../crates/scafra-web/src/routing.rs) constructs actuator
routers even in the default path, reserves enabled actuator aliases during
duplicate detection, obtains policy records, validates security, installs a
1 MiB body limit, installs security middleware, then optional timeout and tracing.
The timeout returns 504. Retain layer ordering and default no-timeout behavior.

[`registration.rs`](../../../crates/scafra-web/src/registration.rs) uses security's
`AuthorizationPolicy` in `ControllerPrefix`. Web re-exports security policy and
inventory types. This is a contract edge as well as an implementation edge.
`authorization_metadata_for` filters linked policies to known routes; moving
records must preserve inventory type identity and verify one effective policy
per route. Keep manual adapters and graph-composed controllers both covered.

`validate_security_config` rejects effective protected routes with disabled
security today. It honors explicit public route overrides. That fail-closed
check must remain present in a web-only build. Private matcher behavior includes
method/HEAD matching, specificity, wildcards and trailing slashes: moving public
declarations is not permission to rewrite policy resolution.

Security's `authentication.rs` imports both JWT and `JwksCache` unconditionally;
`middleware.rs` constructs the cache when installing a layer. JWT validation
uses `jsonwebtoken`; `jwks.rs` uses a reqwest client, request bounds, redirect
policy, discovery, key cache and refresh rules. `config.rs` also uses
`reqwest::Url`, so gating `jwks.rs` alone does not eliminate reqwest. Split local
JWT from remote key acquisition with optional client state, preserving issuer,
audience, scope, algorithm and key validation. Static bearer/Basic should retain
only their own credential parsing/enforcement dependencies.

Actuator's `try_router` and `router_with_health_registry` validate configured
readiness names before serving. Legacy infallible `router` retains the signature
but reports DOWN for invalid selection. Operational bearer security is its own
layer, not application security. Removing the actuator edge from web requires
passing enabled reserved routes and merging the operational router in selected
composition. Axum/Tower already provide the necessary router/layer abstractions.

## Lifecycle, providers and code generation

[`lifecycle.rs`](../../../crates/scafra-core/src/lifecycle.rs) owns synchronous
hook states, deterministic metadata ordering, reverse rollback of started hooks,
and cleanup failures. `ApplicationContext` contains read-only component metadata,
not resource storage. `registration.rs` provides async resource futures and
cooperative shutdown contracts without a Tokio dependency.

[`renderer.rs`](../../../crates/scafra-build/src/renderer.rs) generates typed
fields and constructors, Arc-managed asynchronous outputs, reverse completed
resource cleanup, and Resources ownership. It **always emits HTTP/actuator/
security methods**, including with no controllers/resources. `parser.rs` and
`graph.rs` resolve providers and reject missing, ambiguous, duplicate or cyclic
graphs and invalid resource ownership. Preserve those checks and concrete types.

[`scafra/src/lib.rs`](../../../crates/scafra/src/lib.rs) loads/validates config,
initializes logging, parses address, composes resources/router, starts core hooks,
starts scheduler, then serves. Shutdown callback signals scheduler/resources
before HTTP drain; after serving, scheduler drains, hooks shut down, resources
shut down. Original startup/runtime failures take precedence over cleanup errors,
which are logged with bounded safe metadata. Resource cleanup also covers hook,
scheduler, router and listener failure. Macro main owns graph-construction failure
cleanup. Do not move shutdown into a Drop implementation or cancel provider
futures abruptly.

Main expands through `run_with_resources` and directly reads `config.actuator`,
`config.security` and server timeout. Controller/routes macros emit web paths,
policy constants and inventory records. Service and bean macros use core paths;
`#[bean]` does not implement a trait for its output. Config derive emits config
paths; logger uses foundation. Thus component macros can remain available in
no-web consumers while main/controller/routes require web diagnostics.

## Verified prerequisite contracts

| Issue / integrated commit | Source and regression evidence | Constraint for refactoring |
| --- | --- | --- |
| #78 / `6e2948c` (HEAD merge `e041fc2`) | sources `load_properties`→`load_merged(prefix)`; `scafra/tests/properties_environment.rs`: **22 passed** | Keep prefix wrapping, schema-before-file environment resolution, explicit separators, ambiguity/duplicate rejection, final overrides, redacted errors |
| #75 / `f00e764` | `sources.rs`, config `tests/resolution.rs`, facade `tests/config_resolution.rs` | Preserve explicit source selection, cwd behavior, profile precedence, required-path failures and effective config passed to extensions |
| #74 / `0cd73a0` | bean macro, build parser, `tests/fixtures/{foreign_output,provider_contract,async_owned_dependency}` | External resource output does not require orphan-illegal Bean impl; no implicit blanket/provider output impls; preserve shared resource ownership |
| #41 / `1c7be2d` | core AsyncResource/Set, generated compose/rollback, facade runner, `examples/async-resource`, build graph tests | Retain exact initialization/cleanup ordering, cooperative cancellation and original failure precedence |
| #73 / `ff3526d` | actuator `registry.rs` validation, fallible routing, facade `tests/actuator_startup.rs`, web `tests/actuator_probes.rs` | Unknown readiness names fail startup; graph-scoped registries retain per-application ownership; legacy fallback stays DOWN |

All corresponding workspace tests ran successfully, with ignored tests recorded
in verification. Integration is established by local source/history and tests,
not issue closure status.

## Tooling, CI, release and downstream

CLI templates declare the same facade dependency for web/api/service/monolith;
the `service` starter still defines HTTP health and uses main. Source builds
generate relative local dependencies, installed CLI falls back to registry
`scafra = "0.1"`. Each generated app has an empty `[workspace]` and its own
release profile. CLI dev watches selected config sources; embedded debug
supervision in main watches src/Cargo files and kills/reaps children. Do not
claim this supervisor is the worker runner from #49 or graceful resource shutdown
on reload. Existing generated-project tests build external fixtures; runtime
all-shapes smoke is explicitly ignored by default.

At the Phase 0 audited commit, CI tested workspace all-features only, stable
toolchain, Linux; the recorded MSRV declaration was **1.99.1**. The independent
review lowers the supported declaration to 1.88; see [final review](final-review.md). No approved extra target support is inferred.
Follow-up CI adds selected-feature external processes and pinned MSRV checks.
Current package verification uses `cargo package --list`, which lists files and
does not establish registry buildability. Both package arrays in publish.yml
place macros before build, despite macros→build. A valid current order is:
foundation, core, actuator, scheduler, security, build, macros, config, web,
bootui, cli, facade. Derive/check a topological order after boundary changes;
security→web will change it. Keep workflow repair separate from Phase 0.

The sibling `../ember-test-services` is the existing Scafra-test-services
consumer. Its manifest pins Git `c7f9dd227eb98e02b0882d6264259911956dd7f5`
and adds BootUI. It uses prelude exports, `ScafraConfig`, `web::finish_router`,
direct actuator router construction, config loading and scheduled registration.
Those are concrete compatibility cases. Its script patches the two top-level
Git dependencies to this checkout. Validation here uses a temporary copy and
the same Cargo patches; the sibling working tree is not edited. See baseline
for the exact outcome; a pinned historical build alone would not test HEAD.

## Cycle constraints

- Do not move aggregate config into facade while BootUI consumes it and facade
  optionally depends on BootUI.
- Move declaration ownership **and** remove web→security before adding
  security→web. A temporary pair of those edges is a Cargo cycle.
- Module config owners keep inherent validators; do not add security→config
  while config optionally depends on security. Facade adapters compose them.
- Actuator stays independent of config/web orchestration; accepting Axum and
  its own config avoids actuator→web→actuator.
- Core/foundation must never depend on facade/config/web. Build depends on core;
  core must not depend on build or macros. Rust traits and explicit generated
  composition already solve provider/resource ownership without a new DI engine.
