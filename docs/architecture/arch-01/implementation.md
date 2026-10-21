# ARCH-01 implementation and verification report

This report covers the complete implementation requested after Phase 0. Changes
remain uncommitted on branch
`103-arch-01-make-scafra-framework-modules-independently-opt-in-before-100`, based
on HEAD `e041fc27c46e521e848ecbd3895d85b876c9b831`. No branch switch, commit,
push, merge, PR, publication or issue closure was performed. Phase 0 work was
preserved. [Modules](../../modules.md) is the current public feature/migration
reference; the original audits/baseline remain historical evidence.

## Final architecture

The repository remains a single Cargo workspace with twelve packages. The
facade defaults to core/foundation/config/macros, with explicit optional web,
security, JWT, JWKS, actuator, scheduler and BootUI capabilities. `standard`
retains the old capability set without BootUI. Generic config and core have no
HTTP requirement. Web-only has no security/crypto/client/actuator/scheduler
closure. Cargo features enable actual optional dependencies, not only exports.

The neutral authorization types and inventory records live in foundation, with
compatible web/security re-exports. Optional web adapters preserve integrated
API signatures without a security→web cycle. Config retains its aggregate with
feature-selected fields and compiled schema adapters. Its capability mask
prevents dependency feature unification from enabling config that a runner
cannot enforce. Typed graph generation and ownership remain compile-time Rust.

## Phase-by-phase implementation

| Phase | Implemented files / APIs | Focused verification and remaining limits |
| --- | --- | --- |
| 1 Configuration | config manifest, models/lib/sources; new ConfigCapabilities and loader.capabilities; modular tests and crate README; BootUI forwarding | No-default/all-feature/actuator/JWT config tests and Clippy pass; #78's 22 regressions pass. Generic signatures and #75 precedence preserved. Framework namespaces now reserved for all loaders; deliberate migration |
| 2 HTTP/security | foundation authorization declarations, web registration/routing/server/lib, security policy re-exports; RouterOptions and options-based APIs | Web-only requests, limits/JSON/errors/drain, protected/malformed policies, Basic auth and no-verifier negatives tested; no runtime reflection or replacement DI |
| 3 Actuator | optional web dependency, operational_router, conditional errors/exports and legacy adapters | Actuator-only and integrated tests; real health/live/ready/auth/collision/readiness-failure tests; legacy DOWN and graph-scoped fallible semantics retained |
| 4 JWT/JWKS | security feature manifest, authentication/config/jwt/jwks stubs, feature tests | Basic/static bearer exclude verifier/client; local jwt excludes reqwest; JWT/JWKS tests retain signature/claims/rotation/cache/timeouts. Local issuer restrictions require no remote client |
| 5 Lifecycle | facade startup.rs/private.rs separation, selected scheduler startup/drain, options HTTP callback | Existing hook/provider/resource rollback plus live init/cancel/shutdown/drop and scheduler join; async resource example tests pass. Worker supervision remains #49; BootUI retains runtime-owned extension task semantics |
| 6 Facade | manifest features, lib/prelude exports, conditional error payloads, root compatibility aliases and feature_macros.rs | Default/minimal and each supported selection compiled independently. Mixed subcrate features compile and unavailable runtime intent rejects before providers/listener |
| 7 Macros/graph | macro main/controller/routes gates, renderer core/resources versus selected HTTP items, build fixtures explicit standard, async example manifest | Actual generated consumers pass; no-web main/controller compiler diagnostics; existing graph compile-fail/pass and #74 provider fixtures; no caller-feature cfg guessing |
| 8 CLI | cli/generator/templates/commands/tests, explicit minimal web manifests, validated --features, matching registry version | All four kinds compile/run, seven optional selections compile, no enabled missing capability, no embedded credentials; generation/filesystem safety retained |
| 9 External/live | consumers.py, negative.py, original downstream inspected and temporary migrated copy | Real TCP JSON/auth/signatures/actuator/BootUI, SIGTERM and cleanup, no-HTTP scheduler; negative feature unions. Original downstream unchanged |
| 10 CI | ci.yml independent consumer matrix, MSRV and stable, source prefetch, negative regressions, existing workspace checks | Workflow parses and commands exercised locally. Declared MSRV 1.99.1 retained. Verified target Linux x86_64; hosted execution requires a later push and was not triggered |
| 11 Release | both publish arrays, publish-order checker, package-check staging script, publishing docs | Correct DAG including build before macros; twelve normalized packages verify against local directory registry source; version-only web/standard consumers pass. No actual crates.io propagation/upload claimed |

## Security review and negative guarantees

An independent read-only reviewer audited the configuration, enforcement,
metadata, lifecycle and feature-unification changes. Findings were addressed:

- Narrow config capabilities using the facade's own features before graph or
  listener startup. Test a web facade beside directly enabled config/security,
  actuator and scheduler features; config rejects rather than becoming public.
- Default-complete RouterOptions across crates so dependency feature unions
  cannot cause missing-field compilation failures.
- Local HS256 secret + issuer/audience works without JWKS; issuer discovery and
  remote key intent reject when unavailable.
- Handwritten inherited role/scope requirements require enforcement even under
  permit_all and public controller defaults; insufficient privileges deny.
- Conflicting policy records reject router construction. Direct infallible
  security layering denies contradictory/duplicate policies in either order.
- Explicit HEAD protection takes precedence over GET's public fallback.
- Invalid or unavailable verifier config cannot disappear through the direct
  layer's disabled-security fast path or fall back to Basic/static bearer.
- Pre-logging failures print bounded static capability/config hints, never
  arbitrary error values. Existing Display/Debug/snapshot redaction remains.

Readiness selection failures remain fatal before serving; legacy infallible
actuator routing reports DOWN. Actuator's token guard remains separate from
application authentication. Resource failures retain original-error precedence,
reverse rollback and retained owners. Cancellation callbacks remain idempotent;
multiple cancellation notifications do not imply multiple shutdown owners.

BootUI's old downstream test failed because a Tower request lacked real peer
metadata. The temporary copy now supplies loopback ConnectInfo and checks the
allow-listed snapshot. The guard was preserved; the original repository was
not changed.

## Dependency comparison

Counts exclude the selected facade root and deduplicate package name/version on
normal edges; host proc-macro normal edges are included, build/dev edges are
separate. Recorded trees and external-consumer absence assertions agree.

| Facade selection | Direct normal | Full normal closure | Indirect normal |
| --- | ---: | ---: | ---: |
| Before: mandatory full facade | 13 | 136 | 123 |
| Empty default/minimal | 7 | 40 | 33 |
| web | 10 | 78 | 68 |
| security | 11 | 80 | 69 |
| jwt | 11 | 120 | 109 |
| jwks | 11 | 134 | 123 |
| actuator | 11 | 79 | 68 |
| scheduler | 9 | 49 | 40 |
| bootui | 12 | 81 | 69 |
| standard | 13 | 136 | 123 |
| standard + bootui | 14 | 137 | 123 |

The generated API also depends on serde: its normal closure is 79 with web,
versus 137 previously and 137 with standard. Counts establish isolation; they
are not measurements of linked code, startup or runtime-loaded dependencies.
Manifest declarations include optional edges and therefore are not the same
quantity as selected direct normal counts.

## Compatibility and migration

Select `standard` for existing full applications, `web` for minimal HTTP, and
`bootui` explicitly for dashboard integration. Direct config/security/web
consumers select matching module features. Configuration fields, StartupError
payload variants, root/prelude exports and the HTTP Result alias are conditional.
Exhaustive matches/struct literals must follow feature selection. Unknown
framework fields and unavailable sections now reject across selected sources,
including explicitly disabled sections; custom application namespaces remain.

Root and hidden generated paths are preserved under their supported selections.
Neutral policy records keep one inventory identity through re-exports. Legacy
integrated graph/router helpers retain signatures with actuator/security; new
options helpers work without either. Core lifecycle/resource/provider APIs are
unchanged, including foreign output support without orphan Bean impls. See
[full migration guidance](../../modules.md#api-migration-from-the-previous-facade).

## Build and runtime observations

The before baseline remains in baseline.md/results. After measurements use the
same Rust/Cargo 1.99.1 host, Linux target, CLI API source shape and release
profile (thin LTO, codegen-units 1, unstripped). The archived baseline consumer
lock seeds resolution; only internal edges are refreshed. Common external
package versions remain unchanged; minimal builds remove unselected packages.
Each clean build uses a new target, with warm registry/OS cache. Source and lock
fingerprints are recorded, and the collector rejects source mutation during a
run. Developer-host load and Cargo locks cause noisy single-run times; no
statistical speed/startup/memory improvement is claimed.

Final measurements and exact verification records are stored in
[implementation-results.json](implementation-results.json). Measurements include
clean/no-op/edit/release builds, unstripped size, spawn-to-first HTTP and sampled
/proc RSS/HWM. Numeric tables are populated from the completed collectors below.
These measurements are not cold-cache, throughput, long-lived memory, network
JWKS startup or a production worker benchmark.

## Verification and reproducibility

The six requested commands are run with TMPDIR=/var/tmp to avoid a small tmpfs;
profiles, feature flags and source behavior remain unchanged:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace --all-features
cargo test --workspace --all-features
```

Additional executed checks include all-target/all-feature Clippy; config and
security feature-specific tests; web no-default/security/actuator/all tests;
#78 properties regression; graph pass/fail fixtures; six async-resource example
tests; CLI unit/integration tests, seven generated capability projects and the
explicitly ignored all-kinds TCP/graceful-shutdown smoke; thirteen consumer
selections; five isolated negative/unification cases; twelve package verifications
and registry-source web/standard consumers. Exact results, paths and command
statuses are recorded in the JSON artifact.

Intermediate failures are preserved in external logs: the first CLI parallel
suite exhausted /tmp, then passed under /var/tmp; default workspace first exposed
missing test-only BootUI config features and was corrected without adding normal
runtime dependencies; one malformed test-file prefix was removed after fmt/
Clippy/compiler rejection. The initial downstream 403 was a peer-fixture mismatch.
Measurement snapshots taken before final security hardening are superseded by
fingerprinted final runs. None of these failures is represented as a passing run.
The ignored actuator registration doctest remains ignored; no claim of executing
it is made. GitHub-hosted jobs were not triggered; local equivalents and workflow
syntax were checked. Actual crates.io stable dependencies are not published,
so unpatched online registry resolution is a release-operation limitation.

## Acceptance criteria

Final per-criterion status is recorded in implementation-results.json and the
completion table below. A PASS means implemented and locally verified, not merged
or released. The production worker supervisor (#49), additional target support,
public version 1.0 and actual publication remain outside this implementation.

## Numeric observations

| Measurement | Before full facade | After web | After standard |
| --- | ---: | ---: | ---: |
| Clean dev build (s) | 22.313 | 21.929 | 24.958 |
| Warm no-op (s) | 0.074 | 0.072 | 0.069 |
| Incremental main edit (s) | 0.451 | 0.370 | 0.414 |
| Release build (s) | 36.457 | 39.960 | 31.083 |
| Unstripped release bytes | 6881760 | 3177552 | 6913224 |

| After runtime scenario | Spawn to successful HTTP (ms) | RSS/HWM sample | Exit |
| --- | ---: | --- | ---: |
| web / default | 27.651 | VmHWM:     4680 kB, VmRSS:     4680 kB | 0 |
| standard / default | 30.668 | VmHWM:     6452 kB, VmRSS:     6452 kB | 0 |
| standard / secure | 5.949 | VmHWM:     6332 kB, VmRSS:     6332 kB | 0 |
| standard / secure-actuator | 5.764 | VmHWM:     6356 kB, VmRSS:     6356 kB | 0 |

Each final collector completed with the same source fingerprint, unchanged workspace lock, and no new external package versions relative to the archived baseline. Minimal selection removes unused dependencies. Before/after samples use the same profile/hardware/toolchain but have uncontrolled host/cache/load differences; no speed ratio, statistical memory reduction or startup benefit is claimed.

## Final acceptance table

| Criterion | Status |
| --- | --- |
| Generic configuration independent of optional runtime modules | PASS |
| HTTP without security/actuator/scheduler dependencies | PASS |
| JWT/JWKS dependencies optional | PASS |
| Actuator optional | PASS |
| Scheduler optional | PASS |
| Core lifecycle usable without HTTP | PASS |
| Documented facade Cargo features implemented | PASS |
| Procedural macros support feature combinations | PASS |
| Generated applications include only necessary modules | PASS |
| Protected routes fail closed | PASS |
| Invalid security configuration rejected | PASS |
| Async initialization/rollback/shutdown preserved | PASS |
| CLI templates updated | PASS |
| External consumers pass | PASS |
| Real startup/shutdown tests pass | PASS |
| Dependency isolation independently verified | PASS |
| CI feature matrix implemented and local equivalents verified | PASS |
| Declared MSRV and Linux x86_64 supported target verified | PASS |
| Package/publish DAG verified without publication | PASS |
| API compatibility and migration documented | PASS |
| Before/after measurements recorded | PASS |
| Required documentation updated | PASS |
| All relevant tests pass | PASS |

**COMPLETE:** All requested local implementation and acceptance checks pass. This does not mean merged, published, hosted CI executed, or issue closed.

## Exact required commands and final outcomes

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS (exit 0) |
| `cargo check --workspace` | PASS (exit 0) |
| `cargo test --workspace` | PASS (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (exit 0) |
| `cargo check --workspace --all-features` | PASS (exit 0) |
| `cargo test --workspace --all-features` | PASS (exit 0) |

The final repeated workspace checks ran after source hardening. Targeted verifier tests, packaging and feature-specific checks also pass; earlier failures and excluded release operations are recorded above and in the JSON.

## Changed files

- [.github/workflows/ci.yml](../../../.github/workflows/ci.yml)
- [.github/workflows/publish.yml](../../../.github/workflows/publish.yml)
- [.gitignore](../../../.gitignore)
- [Cargo.lock](../../../Cargo.lock)
- [Cargo.toml](../../../Cargo.toml)
- [README.md](../../../README.md)
- [crates/scafra-bootui/Cargo.toml](../../../crates/scafra-bootui/Cargo.toml)
- [crates/scafra-build/src/renderer.rs](../../../crates/scafra-build/src/renderer.rs)
- [crates/scafra-build/tests/fixtures/ambiguous/Cargo.toml](../../../crates/scafra-build/tests/fixtures/ambiguous/Cargo.toml)
- [crates/scafra-build/tests/fixtures/async_missing_shutdown/Cargo.toml](../../../crates/scafra-build/tests/fixtures/async_missing_shutdown/Cargo.toml)
- [crates/scafra-build/tests/fixtures/async_owned_dependency/Cargo.toml](../../../crates/scafra-build/tests/fixtures/async_owned_dependency/Cargo.toml)
- [crates/scafra-build/tests/fixtures/cycle/Cargo.toml](../../../crates/scafra-build/tests/fixtures/cycle/Cargo.toml)
- [crates/scafra-build/tests/fixtures/duplicate/Cargo.toml](../../../crates/scafra-build/tests/fixtures/duplicate/Cargo.toml)
- [crates/scafra-build/tests/fixtures/foreign_output/Cargo.toml](../../../crates/scafra-build/tests/fixtures/foreign_output/Cargo.toml)
- [crates/scafra-build/tests/fixtures/missing/Cargo.toml](../../../crates/scafra-build/tests/fixtures/missing/Cargo.toml)
- [crates/scafra-build/tests/fixtures/provider_contract/Cargo.toml](../../../crates/scafra-build/tests/fixtures/provider_contract/Cargo.toml)
- [crates/scafra-build/tests/fixtures/qualified/Cargo.toml](../../../crates/scafra-build/tests/fixtures/qualified/Cargo.toml)
- [crates/scafra-cli/src/cli.rs](../../../crates/scafra-cli/src/cli.rs)
- [crates/scafra-cli/src/commands.rs](../../../crates/scafra-cli/src/commands.rs)
- [crates/scafra-cli/src/generator.rs](../../../crates/scafra-cli/src/generator.rs)
- [crates/scafra-cli/src/templates.rs](../../../crates/scafra-cli/src/templates.rs)
- [crates/scafra-cli/src/tests.rs](../../../crates/scafra-cli/src/tests.rs)
- [crates/scafra-cli/tests/generated_projects.rs](../../../crates/scafra-cli/tests/generated_projects.rs)
- [crates/scafra-config/Cargo.toml](../../../crates/scafra-config/Cargo.toml)
- [crates/scafra-config/README.md](../../../crates/scafra-config/README.md)
- [crates/scafra-config/src/lib.rs](../../../crates/scafra-config/src/lib.rs)
- [crates/scafra-config/src/models.rs](../../../crates/scafra-config/src/models.rs)
- [crates/scafra-config/src/sources.rs](../../../crates/scafra-config/src/sources.rs)
- [crates/scafra-config/tests/actuator_security.rs](../../../crates/scafra-config/tests/actuator_security.rs)
- [crates/scafra-config/tests/ember014_matrix.rs](../../../crates/scafra-config/tests/ember014_matrix.rs)
- [crates/scafra-config/tests/modular.rs](../../../crates/scafra-config/tests/modular.rs)
- [crates/scafra-config/tests/scheduler_validation.rs](../../../crates/scafra-config/tests/scheduler_validation.rs)
- [crates/scafra-foundation/Cargo.toml](../../../crates/scafra-foundation/Cargo.toml)
- [crates/scafra-foundation/src/authorization.rs](../../../crates/scafra-foundation/src/authorization.rs)
- [crates/scafra-foundation/src/lib.rs](../../../crates/scafra-foundation/src/lib.rs)
- [crates/scafra-macros/src/controller.rs](../../../crates/scafra-macros/src/controller.rs)
- [crates/scafra-macros/src/lib.rs](../../../crates/scafra-macros/src/lib.rs)
- [crates/scafra-macros/src/main.rs](../../../crates/scafra-macros/src/main.rs)
- [crates/scafra-macros/src/routes.rs](../../../crates/scafra-macros/src/routes.rs)
- [crates/scafra-security/Cargo.toml](../../../crates/scafra-security/Cargo.toml)
- [crates/scafra-security/src/authentication.rs](../../../crates/scafra-security/src/authentication.rs)
- [crates/scafra-security/src/config.rs](../../../crates/scafra-security/src/config.rs)
- [crates/scafra-security/src/jwt.rs](../../../crates/scafra-security/src/jwt.rs)
- [crates/scafra-security/src/lib.rs](../../../crates/scafra-security/src/lib.rs)
- [crates/scafra-security/src/middleware.rs](../../../crates/scafra-security/src/middleware.rs)
- [crates/scafra-security/src/policy.rs](../../../crates/scafra-security/src/policy.rs)
- [crates/scafra-security/tests/config.rs](../../../crates/scafra-security/tests/config.rs)
- [crates/scafra-security/tests/features.rs](../../../crates/scafra-security/tests/features.rs)
- [crates/scafra-security/tests/jwks.rs](../../../crates/scafra-security/tests/jwks.rs)
- [crates/scafra-security/tests/jwt.rs](../../../crates/scafra-security/tests/jwt.rs)
- [crates/scafra-security/tests/policies.rs](../../../crates/scafra-security/tests/policies.rs)
- [crates/scafra-web/Cargo.toml](../../../crates/scafra-web/Cargo.toml)
- [crates/scafra-web/src/errors.rs](../../../crates/scafra-web/src/errors.rs)
- [crates/scafra-web/src/lib.rs](../../../crates/scafra-web/src/lib.rs)
- [crates/scafra-web/src/registration.rs](../../../crates/scafra-web/src/registration.rs)
- [crates/scafra-web/src/routing.rs](../../../crates/scafra-web/src/routing.rs)
- [crates/scafra-web/src/server.rs](../../../crates/scafra-web/src/server.rs)
- [crates/scafra-web/tests/actuator.rs](../../../crates/scafra-web/tests/actuator.rs)
- [crates/scafra-web/tests/actuator_probes.rs](../../../crates/scafra-web/tests/actuator_probes.rs)
- [crates/scafra-web/tests/optional_security.rs](../../../crates/scafra-web/tests/optional_security.rs)
- [crates/scafra-web/tests/security.rs](../../../crates/scafra-web/tests/security.rs)
- [crates/scafra/Cargo.toml](../../../crates/scafra/Cargo.toml)
- [crates/scafra/README.md](../../../crates/scafra/README.md)
- [crates/scafra/src/feature_macros.rs](../../../crates/scafra/src/feature_macros.rs)
- [crates/scafra/src/lib.rs](../../../crates/scafra/src/lib.rs)
- [crates/scafra/src/prelude.rs](../../../crates/scafra/src/prelude.rs)
- [crates/scafra/src/private.rs](../../../crates/scafra/src/private.rs)
- [crates/scafra/src/startup.rs](../../../crates/scafra/src/startup.rs)
- [docs/README.md](../../../docs/README.md)
- [docs/architecture.md](../../../docs/architecture.md)
- [docs/architecture/arch-01/README.md](../../../docs/architecture/arch-01/README.md)
- [docs/architecture/arch-01/adr-001-modular-framework.md](../../../docs/architecture/arch-01/adr-001-modular-framework.md)
- [docs/architecture/arch-01/baseline-consumer-lock.toml](../../../docs/architecture/arch-01/baseline-consumer-lock.toml)
- [docs/architecture/arch-01/baseline-results.json](../../../docs/architecture/arch-01/baseline-results.json)
- [docs/architecture/arch-01/baseline.md](../../../docs/architecture/arch-01/baseline.md)
- [docs/architecture/arch-01/cargo-inventory.md](../../../docs/architecture/arch-01/cargo-inventory.md)
- [docs/architecture/arch-01/compatibility-security.md](../../../docs/architecture/arch-01/compatibility-security.md)
- [docs/architecture/arch-01/dependency-audit.md](../../../docs/architecture/arch-01/dependency-audit.md)
- [docs/architecture/arch-01/feature-matrix.md](../../../docs/architecture/arch-01/feature-matrix.md)
- [docs/architecture/arch-01/implementation-results.json](../../../docs/architecture/arch-01/implementation-results.json)
- [docs/architecture/arch-01/implementation.md](../../../docs/architecture/arch-01/implementation.md)
- [docs/architecture/arch-01/roadmap.md](../../../docs/architecture/arch-01/roadmap.md)
- [docs/configuration.md](../../../docs/configuration.md)
- [docs/contributing.md](../../../docs/contributing.md)
- [docs/generated-code.md](../../../docs/generated-code.md)
- [docs/getting-started.md](../../../docs/getting-started.md)
- [docs/modules.md](../../../docs/modules.md)
- [docs/publishing.md](../../../docs/publishing.md)
- [docs/security.md](../../../docs/security.md)
- [examples/async-resource/Cargo.toml](../../../examples/async-resource/Cargo.toml)
- [scripts/arch103-baseline.py](../../../scripts/arch103-baseline.py)
- [scripts/arch103-consumers.py](../../../scripts/arch103-consumers.py)
- [scripts/arch103-negative.py](../../../scripts/arch103-negative.py)
- [scripts/arch103-package-check.py](../../../scripts/arch103-package-check.py)
- [scripts/arch103-publish-order.py](../../../scripts/arch103-publish-order.py)
