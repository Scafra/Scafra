# Independent review of ARCH-01 (#103)

## A. Executive summary and provenance

The dependency architecture is sound for the generated, declared-prefix graph
composition contract: real optional Cargo edges isolate configuration, core,
HTTP, security, JWT/JWKS, actuator and scheduling without a new container.
However, the original completion checklist was too broad. This review reproduced
three authorization bypasses, a generic configuration regression, and a CI path
error. Corrections below address those cases except transformed manual controller
composition, which remains a high-severity merge concern.

Reviewed commit: `35b267168ec08a8b42fb3053b86dc2f38ce937b5`.
Branch: `103-arch-01-make-scafra-framework-modules-independently-opt-in-before-100`.
Working tree was clean at review start. Correct merge base with `origin/main`:
`e041fc27c46e521e848ecbd3895d85b876c9b831`. `git ls-remote` confirmed those
exact remote main/branch tips. The implementation is committed and pushed;
these corrective changes are uncommitted. No branch switch, commit, push, merge,
PR, publication, issue closure or original downstream modification occurred.

All 95 changed paths were covered across production/config/facade, independent
security/generated-graph review, and CLI/tooling/docs/package review. Lockfiles
and JSON observations were inspected as dependency/provenance artifacts, not
accepted as evidence that tests still pass. Generated Rust was compiled in
independent consumers and ownership fixtures, in addition to renderer inspection.
Fresh results below supersede the historical implementation checklist.

## B. Confirmed findings and corrections

### F1 — Medium: generic loaders reserved framework namespaces

- **Location:** `crates/scafra-config/src/sources.rs`, `load_yaml`, `load_merged`,
  `merge_file`, `merge_environment`, capability/path validation.
- **Evidence/reproduction:** new tests loaded domain `security.classification`
  and `server.region` through `load_yaml::<DomainSettings>()` and typed loading.
  Both failed before the fix with `InvalidValue { path: "security", source_kind:
  "module is unavailable; enable its Cargo feature or remove its configuration" }`.
- **Root cause/impact:** unconditional framework schema checks in generic paths
  rejected valid application types, including `Properties<T>`.
- **Fix:** generic loads use their typed schema. `.framework()` explicitly selects
  per-input framework checks. `.capabilities(...)` also selects them.
  Defaulted `Config::FRAMEWORK_CONFIGURATION` automatically selects checks for
  validated `ScafraConfig` (both profile APIs included). Domain Config impls keep
  the default false; no second parser or runtime type inspection is introduced.
- **Tests:** generic YAML, raw loader, validated domain properties with environment
  and profile overrides; automatic framework rejection/redaction; existing #75/#78.
  The generic unknown-environment assertion is restored to its pre-#103 expected
  diagnostic rather than pretending the domain fixture is framework settings.
- **Status:** fixed; focused default/all-feature configuration tests pass.

### F2 — High: public parameter route overrode inherited static protection

- **Location:** `crates/scafra-security/src/policy.rs`, `RoutePolicyMatcher::new`
  and `matching_policy`.
- **Reproduction:** global bearer security enabled; static `GET /users/me` has
  inherited policy; parameter `GET /users/{id}` is public. Anonymous
  `GET /users/me` returned **200** from the static handler.
- **Root cause:** filtering inherited/inert declarations removed the static
  route's precedence, so the parameter policy authorized a different handler.
  This defect predates #103 but affected the reviewed implementation.
- **Fix:** retain inherited metadata and select the most specific matching path
  before resolving method or fallback policy, matching Axum path-first routing.
- **Verification:** new regression failed with 200 before the fix, passes with
  401 afterward; public parameter request remains 200, valid static bearer 200.
  Independent original live reproduction changed from 200 to 401.

### F3 — High: parameter HEAD policy bypassed a protected static GET

- **Location:** same matcher, especially the new explicit-HEAD selection.
- **Reproduction:** protected `GET /users/me` (implicit HEAD), public explicit
  `HEAD /users/{id}`. Anonymous `HEAD /users/me` returned **200** even though
  Axum chose the static handler.
- **Root cause:** #103 chose any matching HEAD declaration before path specificity.
- **Fix:** choose the winning path first; explicit HEAD only overrides GET within
  that path. Existing same-path public-GET/protected-HEAD behavior remains tested.
- **Verification:** independently reproduced 200 before/401 after; combined
  regression in `crates/scafra-security/tests/features.rs` covers both cases.

### F4 — High: supplied-router serving omitted controller enforcement

- **Location:** `crates/scafra-web/src/server.rs`, `serve_on_with_signal`;
  facade `run_with_router` and compatibility server wrappers delegate here.
- **Reproduction:** web-only application manually registers a generated
  `#[controller("/protected", authenticated)]` using `ControllerRoutes`, then
  returns that router from `run_with_router`. Live anonymous HTTP returned
  **200 private data**; SIGTERM exited 0.
- **Root cause:** `Some(router)` was trusted as already finalized.
- **Fix:** `finish_supplied_router` validates/enforces all linked controller
  declarations before bind. It skips route-collision checking against unused
  linked adapters: Axum already handles collisions in the actual opaque router.
  The normal builders still validate installed route collisions.
- **Verification:** original source now exits before observed listen with
  `protected routes require the security Cargo feature`; a process-level
  regression is included in the nine-case negative suite.
- **Compatibility/cost:** unused linked protected controllers can now reject
  supplied-router startup. Already-finalized supplied routers receive another
  middleware layer, including another independent JWKS cache. This conservative
  correction needs review before freezing the public serving contract.

### F5 — Medium: CI negative suite rejected GitHub's temporary directory

- **Location:** `scripts/arch103-negative.py`, `main`, versus the CI negative step.
- **Evidence:** script required targets under `/var/tmp`; CI passed
  `$RUNNER_TEMP/negative-target`, normally under `/home/runner/work/_temp`.
  Reproduction with an external `/tmp` target exited 2 before running fixtures:
  `Cargo target directory must be under /var/tmp`.
- **Fix:** accept any target outside the checkout, preserving the write-scope
  guard. No unsupported GitHub-hosted success claim is made.
- **Verification:** external `/tmp` target accepted; stand-in Cargo with bounded
  timeouts reached all nine fixtures and recorded the selected target. Deliberate
  compiler timeouts correctly fail the suite, rather than failing at argparse.

### F6 — Low: measurement/test tooling could mask failures or wait indefinitely

- **Location:** baseline runtime shutdown and consumer Cargo subprocess loop.
- **Impact:** baseline recorded nonzero shutdown without failing; consumer Cargo
  commands had no timeout.
- **Fix:** reject nonzero measurement shutdown; add configurable per-Cargo timeout,
  retain logs, and mark timed-out commands failed.
- **Verification:** sleeping Cargo reproduction fails in about 1.3 seconds with
  `--timeout 1`; genuine corrected runtime suites check exit 0. Nonzero benchmark
  shutdown branch was reviewed, not separately fault-injected. Parent timeout
  does not promise process-group termination of every compiler descendant.

### F7 — High, OPEN: transformed manual controller routers lose role/scope policy

- **Location:** `crates/scafra-security/src/middleware.rs`, `authorize_request`
  (`matching_policy` then `policy.unwrap_or_default()`), and
  `scafra-web/src/routing.rs`, `authorization_metadata_for`.
- **Evidence:** rebuilt against current corrections, a generated controller with
  `roles("admin")` nested using `.nest("/v2", controller_router)` returned
  **200 admin data** for a valid shared bearer with no roles; expected **403**.
  Outer `finish_router_with_options` and the corrected standard runner were
  both present. Live SIGTERM exit was 0.
- **Root cause:** URI-based static metadata still describes `/protected`, while
  Axum serves `/v2/protected`. Global authentication succeeds, but the unmatched
  policy becomes default and loses roles/scopes.
- **Reproduction:** independent manifest with facade `security` plus Tokio,
  configuration `security.enabled=true` and a static bearer:

```rust
use scafra::prelude::*;
#[controller("/protected", roles("admin"))]
pub struct Protected;
#[routes]
impl Protected {
    #[get("/")]
    pub async fn secret(&self) -> &'static str { "admin data" }
}
#[tokio::main]
async fn main() {
    scafra::run_with_router(|config| {
        let routes = <Protected as scafra::web::ControllerRoutes>::route_metadata();
        let base = <Protected as scafra::web::ControllerRoutes>::register_routes(
            scafra::web::axum::Router::new());
        let nested = scafra::web::axum::Router::new().nest("/v2", base);
        Ok(scafra::web::finish_router_with_options(
            nested, routes, &scafra::router_options(config))?)
    }).await.unwrap();
}
```

- **Recommendation:** preserve per-route authorization identity/enforcement across
  composition, or reject unsupported transformed controller composition with
  a deterministic fail-closed mechanism. An opaque Router cannot be inspected
  to safely infer arbitrary transformations. Do not add URI-suffix heuristics
  or another global policy registry as a patch.
- **Status:** not fixed; documented unsupported in public API/migration docs.
  Documentation does not satisfy the assignment's broader manual-controller
  enforcement criterion. A focused design/implementation is required before merge.

## C. MSRV decision

#103 did **not** introduce the claimed 1.82→1.99.1 change. The exact merge-base
Cargo.toml already declares 1.99.1; prerequisite #78 commit
`6e2948cbde6c6b1f9e113ebe0669708da35aee37` made that change.

Nevertheless, inherited 1.99.1 was unnecessarily high. The declaration and CI
are corrected to **Rust 1.88** / toolchain **1.88.0**, with unchanged external
Cargo.lock versions. This is the lowest verified compiler for the retained
dependency graph, not a claim that framework syntax needs 1.88.

- Rust 1.82 locked workspace check fails parsing `clap_lex-1.1.1/Cargo.toml`:
  `feature edition2024 is required`. An archive of `6e2948c^`, which still
  declares 1.82, fails identically, proving a pre-existing lock/MSRV problem.
- Selected time 0.3.55/time-core/time-macros, ICU 2.3 and trybuild 1.0.121 require
  1.88. Several others require 1.85/1.86. Metadata identifies these constraints;
  raising to the host's 1.99.1 is unnecessary.
- An isolated copy **does** compile all framework source on 1.82 after Cargo's
  MSRV fallback resolution and explicit downgrades: simple_asn1 0.6.3,
  deranged 0.5.5, idna_adapter 1.2.0, and re-resolved time/ICU and related packages.
  Exact command: `cargo +1.82.0 check --manifest-path
  /var/tmp/scafra-msrv-review/Cargo.toml --workspace --all-features --locked`.
  It passed; there is no newly required language feature.
- Fallback alone does not suffice: simple_asn1 0.6.4 requires time>=0.3.47 (1.88),
  deranged 0.5.9 uses edition2024, and ICU2 resolves zerovec requiring >1.82.
  Maintaining the older graph requires several transitive constraints/lock
  guidance, also for fresh Cargo1.82 consumers. No artificial direct dependency
  pins are added merely to force these old versions.

Rust 1.82 support is therefore **not preserved** as a package-resolution promise.
Consumers needing it must maintain a separately downresolved lock; that is not
this release contract. Maintainers should explicitly approve this compatibility
policy despite the reviewed change lowering the actually inherited requirement.

## D. Configuration compatibility and capability masks

Generic YAML and typed domain configuration accept `security.classification` and
`server.region`; domain validation and Properties environment overrides remain
usable. Raw deserialization is not a framework startup validator. Direct
application-config callers should migrate to
`load_validated::<ScafraConfig>()`, or explicitly select `.framework()` when
using raw load methods. The new Config associated constant defaults to false,
so existing domain Config impls need no source change.

Framework checked loading validates selected inputs **before merging**, so an
override cannot erase unavailable security intent. The runner selects capability
flags from its facade features, independent of unified subcrate features, before
provider initialization/listen. Tests now directly enable config security,
actuator, scheduler, web security, and security JWT/JWKS beside web-only facade;
all corresponding configurations reject without resource events/listener. A
security facade beside direct JWKS also cannot accept JWT configuration.
Unknown framework fields, profiles, explicit source failures, redaction,
precedence and #78's 22 property regressions remain covered.

## E. Security and lifecycle assessment

Declared-prefix generated composition and complete-metadata finalization cover
public/protected controllers/routes, public overrides, role/scope requirements,
duplicate/conflicting declarations, GET/HEAD and static/parameter precedence.
Security absent/disabled rejects protection; invalid verifier settings cannot
fall back to Basic/static bearer. Fresh security tests cover JWT signatures,
algorithm/issuer/audience/expiry and 14 actual JWKS discovery/cache/rotation/
network tests. Actuator wrong/missing token tests use real TCP; unknown readiness
checks reject startup. BootUI peer guard remains intact.

Arbitrary handwritten Axum routes do not acquire inferred controller policy.
Raw registration adapters only install handlers; explicit finalization requires
complete untransformed metadata. F4 strengthens Scafra server boundaries, but
F7 means no unconditional manual-transformation guarantee can be made.

Renderer and actual compile fixtures confirm router-only helpers compile without
managed async resources and are absent (E0599) when owners exist, for options and
compatibility APIs. Router errors retain Resources for cleanup; generated main
cleans them up. Existing reverse-order rollback, original-error preservation,
cooperative cancellation, scheduler join and HTTP-drain ordering remain intact.
Actual async-resource example passes six tests. BootUI still ends with its owning
runtime; separately graceful dashboard drain and worker supervision remain
outside #103's implemented contract, not newly invented guarantees.

## F. Cargo architecture and independent dependency accounting

Defaults are empty. Supported additive facade capabilities remain `web`,
`security`→web, `jwt`→security, `jwks`→jwt, `actuator`→web,
`scheduler` without HTTP, `bootui`→security, and `standard` containing
web/security/JWT/JWKS/actuator/scheduler but excluding BootUI.

Fresh independent normal-edge counts reproduce:

| Selection | Direct normal | Normal closure excluding facade root |
| --- | ---: | ---: |
| Minimal/default | 7 | 40 |
| Web | 10 | 78 |
| Standard | 13 | 136 |

Counts deduplicate package name/version; they include proc-macro **normal** host
edges. They exclude build/dev edges and are not runtime-loaded library counts.
Build/proc-macro compilation costs remain even when HTTP is excluded. Separate
external `[workspace]` consumer roots prevent workspace feature unification from
hiding isolation failures. Shared target caches do not change Cargo feature
selection between independent invocations. Features are unions, not subtraction.

## G. Public API migration

- Bare `scafra = "0.1"` is intentionally breaking: select `standard` for the old
  capability set, add `bootui` if used, or select minimal required capabilities.
  This restores capabilities, **not full source compatibility**.
- Conditional ScafraConfig fields, StartupError variants, facade/prelude exports
  and web-only `scafra::Result` require matching selections and source edits to
  struct literals/exhaustive matches.
- `standard` excludes the former always-present `ScafraConfig.bootui` field;
  code accessing it must select `bootui`, even if runtime use is disabled.
- Direct config/web/security consumers select matching subcrate features;
  coordinated macro/build/facade versions preserve generated paths.
- Generic loader compatibility is restored (F1); framework startup should use
  validated aggregate APIs. Existing domain Config implementations remain valid.
- Supplied-router APIs now finalize all linked controller policies; unused linked
  protected declarations can reject startup, and middleware may be repeated.
  Untransformed graph composition is the verified contract; F7 prevents broader
  nesting guarantees.
- CLI defaults remain explicit web-only for all four kinds; optional selections
  never generate hardcoded credentials or config for absent capabilities.
  The `service` starter is still HTTP, not a complete worker implementation.

## H. Fresh verification

The following commands were actually executed with `TMPDIR=/var/tmp` on
Linux x86_64. Stable compiler/Cargo is 1.99.1; MSRV checks use 1.88.0.
All seven required workspace commands passed on the corrected source. Fresh
performance observations and final acceptance evaluation follow.

| Command / scenario | Fresh result and evidence |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace` | PASS, final rerun log `/var/tmp/scafra-final-review-checks/check-final.log` |
| `cargo test --workspace` | PASS final-source rerun, `/var/tmp/scafra-final-review-checks/workspace-rerun.log`; earlier intermediate failure retained |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo check --workspace --all-features` | PASS |
| `cargo test --workspace --all-features` | PASS, `/var/tmp/scafra-final-review-checks/5.log` |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test -p scafra-config` and `--all-features` | PASS |
| `cargo test -p scafra --features standard --test properties_environment` | PASS, 22 #78 regressions |
| `cargo test -p scafra-security --all-features --offline` | PASS, 48 security tests |
| `cargo test -p scafra-security --no-default-features --offline` | PASS, 21 tests |
| `cargo test -p scafra-web --no-default-features --test optional_security --offline` | PASS, 5 tests |
| `cargo test -p scafra-web --all-features` | PASS after supplied-router helper correction |
| `cargo test --manifest-path examples/async-resource/Cargo.toml --offline` | PASS, 6 generated resource tests |
| `cargo +1.88.0 check --workspace --all-features --all-targets --locked` | PASS on actual corrected root (target cache external) |
| `cargo +1.88.0 test --manifest-path /var/tmp/scafra-msrv188-review/Cargo.toml --workspace --all-features --locked -- --test-threads=1` | PASS, current-source byte-identical isolated snapshot; 369 passed, 2 ignored; `/var/tmp/msrv-188-test-final.log` |
| `RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/arch103-consumers.py OUTPUT --case minimal --runtime` | PASS |
| Same MSRV consumer command with `--case standard --runtime` | PASS, real authentication/actuator/scheduler/resources |
| `python3 scripts/arch103-consumers.py /var/tmp/scafra-review-consumers-confirmed --runtime` | PASS all 13, no forbidden dependencies; actual HTTP/SIGTERM/resource checks |
| `python3 scripts/arch103-negative.py --output /var/tmp/scafra-review-negative-final-source` | PASS all 9, including generated ownership and supplied protected router |
| `cargo test -p scafra-cli --test generated_projects cli_generates_compiling_optional_capability_projects -- --exact` | PASS, 7 independently generated manifests |
| `cargo test -p scafra-cli --test generated_projects generated_projects_serve_routes_and_shutdown_gracefully -- --exact --ignored` | PASS final-source repeat, all 4 generated kinds over TCP/SIGTERM; `/var/tmp/scafra-review-cli-runtime-final.log` |
| `python3 scripts/arch103-publish-order.py --check` | PASS, both 12-crate orders, build before macros |
| `python3 scripts/arch103-package-check.py /var/tmp/scafra-arch103-packages-20261021 --offline --reuse` | PASS fresh corrected-source 12 verified archives and version-only web/standard macro consumers; `/var/tmp/scafra-review-packages-confirmed.log` |
| `cargo test --manifest-path /var/tmp/scafra-103-downstream-modular/Cargo.toml --offline` | PASS after temporary path/feature migration and fixture corrections; original downstream remains unchanged |
| Python AST/JSON parsing and `git diff --check` | PASS |

Normal workspace runs retain intentional ignored tests; the actual four-kind
CLI runtime test is explicitly invoked separately. A temporary migration enables
standard+BootUI, replaces old git sources with current local paths, supplies
loopback ConnectInfo in the BootUI in-memory fixture and tests its allow-listed
snapshot. It does not weaken the dashboard peer guard or modify the original
`../ember-test-services` checkout.

Intermediate failures are recorded, not hidden: F1 and the static policy test
failed before correction; the first supplied-router implementation checked
unused linked collision fixtures and broke the drain test, leading to the
separate authorization-only supplied-router finalizer. One early MSRV snapshot
had new config code with the old diagnostic assertion; it was resynchronized.
Consumer benchmark attempts rejected changing-source fingerprints before a final
frozen run. These earlier snapshots do not count as successful final checks.

Runtime port reservation releases before bind, so concurrent port conflict is
possible and fails visibly. Negative listener checks poll loopback; source
ordering, not polling alone, establishes validation before bind. Scripts are
Linux/Unix verification, not a Windows/macOS support claim. Actual GitHub-hosted
CI and crates.io publication/index propagation remain unverified release steps.

## I. Corrective files

The following files changed during this review (relative to reviewed commit):

- `.github/workflows/ci.yml`
- `Cargo.toml`
- `crates/scafra-config/README.md`
- `crates/scafra-config/src/models.rs`
- `crates/scafra-config/src/sources.rs`
- `crates/scafra-config/tests/ember014_matrix.rs`
- `crates/scafra-config/tests/modular.rs`
- `crates/scafra-security/src/policy.rs`
- `crates/scafra-security/tests/features.rs`
- `crates/scafra-web/src/registration.rs`
- `crates/scafra-web/src/routing.rs`
- `crates/scafra-web/src/server.rs`
- `crates/scafra/src/startup.rs`
- `docs/architecture/arch-01/README.md`
- `docs/architecture/arch-01/adr-001-modular-framework.md`
- `docs/architecture/arch-01/cargo-inventory.md`
- `docs/architecture/arch-01/dependency-audit.md`
- `docs/architecture/arch-01/final-review.md`
- `docs/architecture/arch-01/implementation.md`
- `docs/architecture/arch-01/roadmap.md`
- `docs/configuration.md`
- `docs/getting-started.md`
- `docs/modules.md`
- `docs/publishing.md`
- `scripts/arch103-baseline.py`
- `scripts/arch103-consumers.py`
- `scripts/arch103-negative.py`

The code corrections are limited to config mode selection, policy path/method
precedence and supplied-router enforcement. Other changes are regression tests,
MSRV/CI alignment, tooling checks and documentation. Cargo.lock external versions
are unchanged. Historical baseline JSON/lock observations retain their original
compiler/revision; they are not rewritten to manufacture current results.

## J. Release readiness

The architecture and corrections can be reviewed, but **merge is not recommended**
while F7 remains reproducible and supplied-router finalization tradeoffs lack a
settled contract. A 1.0 release also requires explicit MSRV/API policy approval,
actual hosted CI, and the separate release checklist/RC process (#16/#33/#42/#39).
No claim of merge, publication or issue completion is made.

## K. Acceptance criteria

| #103 criterion | Fresh status | Basis / remaining work |
| --- | --- | --- |
| Generic configuration independent of optional runtime modules | PASS | Isolated direct config graph, F1 domain regressions |
| HTTP without security/actuator/scheduler dependencies | PASS | Web normal closure 78, forbidden-package assertions and live HTTP |
| JWT/JWKS dependencies optional | PASS | Separate security/JWT/JWKS consumers and negative verifier tests |
| Actuator optional | PASS | Web absence and actuator-only authenticated operational requests |
| Scheduler optional | PASS | Web absence and HTTP-free scheduler execution/drain |
| Core lifecycle usable without HTTP | PASS | Isolated core/minimal process fixtures |
| Facade feature contract implemented | PASS | Empty defaults and all 13 isolated cases |
| Macros support selected features | PASS | Actual generated code and feature/owner compile-fail fixtures |
| Generated applications include required modules only | PASS | Seven profiles, four CLI application kinds |
| Protected routes fail closed in all requested composition paths | FAIL | F7 live nested controller roles bypass remains 200 |
| Invalid security configuration rejected | PASS | Capability-union negatives before providers/listener, verifier tests |
| Async initialization/rollback/shutdown preserved | PASS | Actual generated resources, six example tests and live cancellation/events |
| CLI templates updated | PASS | Minimal/selected manifests, no hardcoded secrets, live generated binaries |
| External consumers pass | PARTIAL | Standard 13 pass; additional transformed-controller reproduction fails expected403 |
| Real startup/shutdown tests pass | PASS | Actual TCP, SIGTERM, exit0, resource cleanup and scheduler drain |
| Dependency isolation independently verified | PASS | Minimal40/web78/standard136 and absent-package checks |
| CI validates supported features | PARTIAL | Matrix implemented, CI path defect fixed, local equivalents pass; hosted CI not verified |
| MSRV/supported target verified | PASS | Rust1.88 current-source tests/checks and Linux x86_64 runtime; no extra target claim |
| Package/publish dependency ordering correct | PASS | Twelve extracted packages, version-only web/standard consumers, DAG checks |
| API compatibility/migration documented | PARTIAL | Breaking features/config modes documented; supplied-router/nesting enforcement contract needs resolution |
| Before/after measurements recorded | PASS | Frozen web measurement below with exact retained external versions |
| Required documentation updated | PASS | ADR/modules/config/MSRV/status corrected; historical checklist explicitly superseded |
| All relevant tests pass | FAIL | Seven Cargo checks and supported suites pass, but F7 manual live security test fails |

## Repeated performance observation

Executed:

```bash
python3 scripts/arch103-baseline.py /var/tmp/scafra-review-baseline-confirmed-web \
  --measure --features web \
  --consumer-lock docs/architecture/arch-01/baseline-consumer-lock.toml \
  --refresh-consumer-lock
```

| Observation | Historical full baseline | Corrected web-only repetition |
| --- | ---: | ---: |
| Direct facade normal dependencies | 13 | 10 |
| Facade normal closure, excluding facade | 136 | 78 |
| Generated app normal closure, excluding app | 137 | 79 |
| Clean dev build | 22.312693 s | 40.190635 s |
| Warm no-op | 0.073891 s | 0.571227 s |
| Source-edit incremental build | 0.451492 s | 1.448832 s |
| Release build | 36.457146 s | 175.062158 s |
| Unstripped release binary | 6,881,760 B | 3,178,584 B |
| Spawn to first successful HTTP | 6.43–30.16 ms observed | 39.245496 ms, one observation |
| Sampled RSS | 6,284–6,432 kB | 4,616 kB |
| Shutdown exit | 0 | 0 |

Both use Rust/Cargo1.99.1, Linux x86_64, CLI API template, dev defaults and
release thin LTO/codegen-units1. Hardware: Intel i7-11370H, 8 logical CPUs.
The review ran concurrent Cargo/CLI/MSRV builds on the host, including package-cache
waits. Timings are **not** a controlled speed comparison; the slower observations
are recorded rather than hidden or called improvements. Startup/RSS are single
samples, not guarantees. Binary size and dependency counts are concrete artifact
observations; this does not promise the same reductions in every application.
A corrected standard binary was not re-benchmarked; its dependency closure was
independently reproduced.

`report.json` and `lock-comparison.json` in the output directory record the
methodology. Seventy-eight retained external registry package version/source/
checksum identities match the archived lock; zero were added/changed,92 unused
ones were removed. Workspace lock SHA256 remained
`f82b02aebbd2d89b9a43be19cb163551089f440826a94b6dabd79f50f6584141`.
Framework source fingerprint remained
`8c65aabcbab9754d7bb217ff9ebb817223bb9a7f60e21bf9d5588155396b4589`.
The measured revision is reviewed HEAD plus the uncommitted corrections listed
above, not an unmodified build of35b2671. No statistical performance claim is made.

## Required continuation before merge

Resolve F7 with request/route-local authorization identity or another explicitly
reviewed fail-closed composition contract. Preserve the failing nested-controller
reproduction as a regression proving403 for valid credentials lacking roles,
then cover scopes, public overrides, HEAD, missing enforcement and post-finalization
nesting. Do not infer transformed policies with permissive URI heuristics.
Review the duplicated supplied-router middleware/JWKS-cache tradeoff alongside
that fix, then rerun the isolated/process and MSRV matrix and hosted CI.
This is a security contract correction, not a request to replace typed DI or
introduce a global registry. Issue#103 is not declared resolved.

**CHANGES REQUIRED BEFORE MERGE**
