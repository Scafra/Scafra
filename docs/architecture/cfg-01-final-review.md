# CFG-01 final implementation and independent review

Decision: **READY FOR PR REVIEW.**

The initial implementation was critically reassessed and three additional defects
were reproduced and corrected: missing programmatic standard-startup injection,
bind-failure cancellation ordering, and ConfigLoader Debug secret exposure.
Refreshed workspace/MSRV, generated runtime, isolated consumer, negative and
packaged-consumer checks pass. Independent follow-up review reports no unresolved
confirmed blocker. Hosted CI and maintainer approval remain separate. No agent commit,
push, PR, publication or remote issue change was performed.

## A. Repository state

- Branch: `111-cfg-01-audit-all-scafra-defaults-and-make-operational-settings-overridable-before-100`.
  The task owner corrected the original `11-` spelling explicitly.
- Original implementation base: `d2bb06be59e98db22e6c7d1b96181395786e3376`.
- This correction pass began with a clean tree at `8ddf27d2e871cb34312f68de3c4584ed64356eac`.
  Existing committed implementation was preserved; follow-up corrections remain uncommitted.
- Combined changed paths relative to the original base: **84**.
- Final verification fingerprint of tracked/untracked nonignored Rust files,
  Cargo manifests and lockfile: `4728954d02ef681635c40d2a59535986006ac31d2dceede2ef6e3c80befdfcbe`. This broader file-set hash
  replaces the earlier changed-files-only fingerprint; the hashes are not comparable.
- No branch creation/switch, agent commit, push, PR, merge, package publication,
  version change, remote issue edit or unrelated repository edit.
- AGENTS-referenced repository skills were absent; the task owner explicitly
  authorized continuing without them. No absent skill is claimed as loaded.

Combined implementation/verification paths (logs remain ignored):

- [Cargo.lock](../../Cargo.lock)
- [Cargo.toml](../../Cargo.toml)
- [README.md](../../README.md)
- [crates/scafra-actuator/src/config.rs](../../crates/scafra-actuator/src/config.rs)
- [crates/scafra-actuator/src/registry.rs](../../crates/scafra-actuator/src/registry.rs)
- [crates/scafra-actuator/src/routing.rs](../../crates/scafra-actuator/src/routing.rs)
- [crates/scafra-actuator/tests/config.rs](../../crates/scafra-actuator/tests/config.rs)
- [crates/scafra-actuator/tests/registry.rs](../../crates/scafra-actuator/tests/registry.rs)
- [crates/scafra-actuator/tests/security.rs](../../crates/scafra-actuator/tests/security.rs)
- [crates/scafra-bootui/Cargo.toml](../../crates/scafra-bootui/Cargo.toml)
- [crates/scafra-bootui/src/binding.rs](../../crates/scafra-bootui/src/binding.rs)
- [crates/scafra-bootui/src/lib.rs](../../crates/scafra-bootui/src/lib.rs)
- [crates/scafra-bootui/tests/bootui_http.rs](../../crates/scafra-bootui/tests/bootui_http.rs)
- [crates/scafra-build/src/renderer.rs](../../crates/scafra-build/src/renderer.rs)
- [crates/scafra-build/tests/fixtures/configured_shutdown/Cargo.toml](../../crates/scafra-build/tests/fixtures/configured_shutdown/Cargo.toml)
- [crates/scafra-build/tests/fixtures/configured_shutdown/src/main.rs](../../crates/scafra-build/tests/fixtures/configured_shutdown/src/main.rs)
- [crates/scafra-build/tests/fixtures/configured_shutdown/src/main/providers.rs](../../crates/scafra-build/tests/fixtures/configured_shutdown/src/main/providers.rs)
- [crates/scafra-build/tests/graph_compile_pass.rs](../../crates/scafra-build/tests/graph_compile_pass.rs)
- [crates/scafra-cli/src/cli.rs](../../crates/scafra-cli/src/cli.rs)
- [crates/scafra-cli/src/commands.rs](../../crates/scafra-cli/src/commands.rs)
- [crates/scafra-cli/src/filesystem.rs](../../crates/scafra-cli/src/filesystem.rs)
- [crates/scafra-cli/src/templates.rs](../../crates/scafra-cli/src/templates.rs)
- [crates/scafra-cli/src/tests.rs](../../crates/scafra-cli/src/tests.rs)
- [crates/scafra-cli/tests/dev_configuration.rs](../../crates/scafra-cli/tests/dev_configuration.rs)
- [crates/scafra-config/Cargo.toml](../../crates/scafra-config/Cargo.toml)
- [crates/scafra-config/src/lib.rs](../../crates/scafra-config/src/lib.rs)
- [crates/scafra-config/src/merge.rs](../../crates/scafra-config/src/merge.rs)
- [crates/scafra-config/src/models.rs](../../crates/scafra-config/src/models.rs)
- [crates/scafra-config/src/sources.rs](../../crates/scafra-config/src/sources.rs)
- [crates/scafra-config/tests/debug_redaction.rs](../../crates/scafra-config/tests/debug_redaction.rs)
- [crates/scafra-config/tests/operational.rs](../../crates/scafra-config/tests/operational.rs)
- [crates/scafra-config/tests/scheduler_names.rs](../../crates/scafra-config/tests/scheduler_names.rs)
- [crates/scafra-macros/src/lib.rs](../../crates/scafra-macros/src/lib.rs)
- [crates/scafra-macros/src/main.rs](../../crates/scafra-macros/src/main.rs)
- [crates/scafra-macros/tests/ui/main_duplicate_loader.rs](../../crates/scafra-macros/tests/ui/main_duplicate_loader.rs)
- [crates/scafra-macros/tests/ui/main_duplicate_loader.stderr](../../crates/scafra-macros/tests/ui/main_duplicate_loader.stderr)
- [crates/scafra-macros/tests/ui/main_loader_call.rs](../../crates/scafra-macros/tests/ui/main_loader_call.rs)
- [crates/scafra-macros/tests/ui/main_loader_call.stderr](../../crates/scafra-macros/tests/ui/main_loader_call.stderr)
- [crates/scafra-macros/tests/ui/main_loader_missing_value.rs](../../crates/scafra-macros/tests/ui/main_loader_missing_value.rs)
- [crates/scafra-macros/tests/ui/main_loader_missing_value.stderr](../../crates/scafra-macros/tests/ui/main_loader_missing_value.stderr)
- [crates/scafra-macros/tests/ui/main_unknown_option.rs](../../crates/scafra-macros/tests/ui/main_unknown_option.rs)
- [crates/scafra-macros/tests/ui/main_unknown_option.stderr](../../crates/scafra-macros/tests/ui/main_unknown_option.stderr)
- [crates/scafra-security/src/config.rs](../../crates/scafra-security/src/config.rs)
- [crates/scafra-security/src/jwks.rs](../../crates/scafra-security/src/jwks.rs)
- [crates/scafra-security/src/jwt.rs](../../crates/scafra-security/src/jwt.rs)
- [crates/scafra-security/src/lib.rs](../../crates/scafra-security/src/lib.rs)
- [crates/scafra-security/src/middleware.rs](../../crates/scafra-security/src/middleware.rs)
- [crates/scafra-security/tests/config.rs](../../crates/scafra-security/tests/config.rs)
- [crates/scafra-security/tests/jwks.rs](../../crates/scafra-security/tests/jwks.rs)
- [crates/scafra-security/tests/jwt.rs](../../crates/scafra-security/tests/jwt.rs)
- [crates/scafra-web/Cargo.toml](../../crates/scafra-web/Cargo.toml)
- [crates/scafra-web/src/connection.rs](../../crates/scafra-web/src/connection.rs)
- [crates/scafra-web/src/lib.rs](../../crates/scafra-web/src/lib.rs)
- [crates/scafra-web/src/limits.rs](../../crates/scafra-web/src/limits.rs)
- [crates/scafra-web/src/routing.rs](../../crates/scafra-web/src/routing.rs)
- [crates/scafra-web/src/server.rs](../../crates/scafra-web/src/server.rs)
- [crates/scafra-web/src/tests.rs](../../crates/scafra-web/src/tests.rs)
- [crates/scafra-web/tests/optional_security.rs](../../crates/scafra-web/tests/optional_security.rs)
- [crates/scafra-web/tests/resource_limits.rs](../../crates/scafra-web/tests/resource_limits.rs)
- [crates/scafra/src/lib.rs](../../crates/scafra/src/lib.rs)
- [crates/scafra/src/prelude.rs](../../crates/scafra/src/prelude.rs)
- [crates/scafra/src/private.rs](../../crates/scafra/src/private.rs)
- [crates/scafra/src/startup.rs](../../crates/scafra/src/startup.rs)
- [crates/scafra/tests/actuator_startup.rs](../../crates/scafra/tests/actuator_startup.rs)
- [crates/scafra/tests/config.rs](../../crates/scafra/tests/config.rs)
- [crates/scafra/tests/foundation_api.rs](../../crates/scafra/tests/foundation_api.rs)
- [crates/scafra/tests/operational_config.rs](../../crates/scafra/tests/operational_config.rs)
- [crates/scafra/tests/startup_bind_cleanup.rs](../../crates/scafra/tests/startup_bind_cleanup.rs)
- [crates/scafra/tests/startup_loader.rs](../../crates/scafra/tests/startup_loader.rs)
- [docs/README.md](../../docs/README.md)
- [docs/api-stability.md](../../docs/api-stability.md)
- [docs/architecture/cfg-01-design.md](../../docs/architecture/cfg-01-design.md)
- [docs/architecture/cfg-01-final-review.md](../../docs/architecture/cfg-01-final-review.md)
- [docs/architecture/configurability-audit.md](../../docs/architecture/configurability-audit.md)
- [docs/configuration-reference.md](../../docs/configuration-reference.md)
- [docs/configuration.md](../../docs/configuration.md)
- [docs/guides/resource-lifecycle.md](../../docs/guides/resource-lifecycle.md)
- [docs/http-errors.md](../../docs/http-errors.md)
- [docs/modules.md](../../docs/modules.md)
- [docs/security.md](../../docs/security.md)
- [docs/upgrading.md](../../docs/upgrading.md)
- [scripts/arch103-negative.py](../../scripts/arch103-negative.py)
- [scripts/issue111-runtime.py](../../scripts/issue111-runtime.py)
- [scripts/issue111-startup.py](../../scripts/issue111-startup.py)

## B. Audit results

The [inventory](configurability-audit.md) audits **all twelve crates** and traces
configuration through its owning types to actual consumers and tests.

- **47 ordinary runtime property schemas**, plus one loader-owned profile selector.
  Dynamic task fields count once per schema, not once per registered job.
- **32** ordinary keys already existed; **15** runtime override keys were added.
- **16 missing override surfaces corrected**: those 15 runtime keys plus the
  separate CLI/embedded supervisor polling control. A changed timeout default
  and validation/wiring defects are not counted as additional new keys.
- **30 notable candidates/controls** classified separately: 4 COMPILE_TIME,
  10 FIXED_INVARIANT, 9 INTERNAL, 5 CLI_OR_BUILD, 2 additional configuration controls.
  Thus 14 candidate rows are fixed/compile-time exceptions. Concrete reasons
  are in the inventory; private constants are not indiscriminately publicized.

Additional corrected defects include timeout-alias precedence, sequence-valued
properties/environment overrides, positional-sequence capability bypasses,
exotic scheduler names, ignored late profile selectors, unsafe BootUI mount/bind
policies, partial Basic credentials, and actuator validation. Existing module
configuration was reused; the audit did not assume that an existing struct
alone proved consumption. The three reassessment defects are additional wiring,
lifecycle and redaction corrections, not additional configuration keys; counts
are unchanged. Their evidence is in the inventory's critical reassessment table.

## C. Configuration model

The existing ConfigLoader remains the only loader. Typed defaults are merged
with base YAML → YML → properties, then the same profile extension order,
environment, then explicit overrides. Validation runs before standard startup
creates resources/listeners. Values are immutable startup settings; restart
rather than recompilation applies ordinary changes. Custom embedding explicitly
passes typed configuration to its consumers.

`server.timeout` remains canonical; `request_timeout_seconds` remains an input
alias. Alias normalization occurs per source, duplicate spellings reject, and
later sources retain precedence. Framework namespaces and capabilities validate
before merging so a later override cannot erase unavailable security intent.
Sequences are accepted only at schema-declared framework list leaves, preventing
Serde positional struct decoding from hiding gated settings. Generic domain
loaders retain their independent schema/path behavior.

Schema-aware environment paths preserve underscores. For example:
`SCAFRA_SERVER__MAX_REQUEST_BODY_BYTES=2048`. Dynamic scheduler names preserve
literal case and punctuation in files/programmatic paths. Explicit
`TASK_HEX_<name bytes>` environment components represent otherwise ambiguous
names; ambiguity and duplicate canonical targets reject. Profile selection is
an earlier loader phase: use a base-file selector, `.profile()` or SCAFRA_PROFILE.
Selectors in other profile files or late property/environment overrides reject
instead of being accepted and ignored. An explicitly selected base file retains
its base role when the profile extension pass reapplies that same file, including
canonical path aliases; existing extension precedence is preserved.

No runtime reflection, service locator, mutable global configuration registry
or second loader was introduced. New startup/connection state has explicit
owners. `run_with_loader` and `run_with_loader_and_resources` accept the caller's
loader; `#[scafra::main(config_loader = path)]` invokes its function once after
the original main body. A single validated result reaches address/logging,
composition, scheduler, BootUI and cleanup. Capability masks only narrow. See the [design](cfg-01-design.md) for trade-offs and review corrections.

## D. Defaults and overrides

The [canonical reference](../configuration-reference.md) documents every key,
type, unit, enforced bounds, feature, consumer, risk and valid/invalid example.
All ordinary runtime rows support the six sources described above.

| Added or changed property | Effective default | Scope |
| --- | --- | --- |
| `server.timeout` | 30 seconds; explicit null disables | Processing through response creation; existing key, corrected default |
| `server.max_request_body_bytes` | 1048576 bytes | Extractors plus raw/streamed consumed data |
| `server.header_read_timeout_seconds` | 10 seconds | Managed HTTP/1 header completion |
| `server.idle_timeout_seconds` | 60 seconds | Managed transport inactivity |
| `server.max_connections` | 1024 | Admission per listener; permit retained through upgrade IO |
| `server.max_header_bytes` | 32768 bytes | HTTP/1 parser buffer / HTTP/2 decoded header list |
| `server.http1_keep_alive` | true | Managed HTTP/1 reuse |
| `server.http1_max_headers` | 100 fields | Managed HTTP/1 header count |
| `server.http2_max_concurrent_streams` | 200 | Managed HTTP/2 streams per connection |
| `shutdown.grace_period_seconds` | 30 seconds | Policy-aware HTTP/resource cleanup |
| `shutdown.force_after_grace` | true | Explicit false permits unbounded policy-aware waiting |
| `security.jwt.leeway_seconds` | 60 seconds | Validated 0–300 second claim skew |
| `security.jwt.cache_ttl_seconds` | 300 seconds | Remote key freshness |
| `security.jwt.refresh_cooldown_seconds` | 30 seconds | Remote refresh attempt spacing |
| `security.jwt.connect_timeout_ms` | 2000 milliseconds | Remote provider connect deadline |
| `security.jwt.request_timeout_ms` | 5000 milliseconds | Total provider operation deadline |

The 30-second processing budget is a deployment convention for ordinary
interactive services aligned with the existing 30-second drain budget. It bounds
request occupancy; it is not a measured workload SLA. Body default remains 1 MiB.
Connections/header budgets are finite and explicitly overridable; applications
must size connection × stream × body budgets for their resources. They are not
a whole-process memory quota.

Existing defaults remain explicit in the reference: local application bind
127.0.0.1:8080, logging info/backtrace off, startup banner/config display true,
authentication off with no credentials, actuator endpoint selection empty and
1000 ms/check, scheduler off with enabled tasks inheriting registration intervals,
and BootUI off at 127.0.0.1:8091 `/bootui`, local-only true. Existing logging,
security, health, scheduler and dashboard consumers are traced and exercised.
CLI polling is separately 500 ms (100–60000), flag → environment → default.

## E. Resource protection

The streaming limiter reuses `http-body-util::Limited`; it does not buffer or
pre-drain the request. A small observer records limit failure, immediately drops
and fuses the failed body, and changes a not-yet-committed handler response to
the stable JSON HTTP 413 even if the handler swallowed that error. Known oversized
length/hints reject early; actual consumed bytes are always counted.

Unread unknown-length bodies cannot be classified without reading them. A handler
that stops early can return normally. If a body is forwarded in a streaming
response, headers may precede a later limit error; the stream terminates and its
already committed status cannot become 413. Hyper owns Content-Length framing.
There is no universal wire-byte/413 promise. Tests cover both boundaries.

Request timers retain deadline-first ties, handler-future cancellation and JSON
504 `request_timeout`. A route ceiling can tighten, but cannot raise/remove an
outer body or deadline ceiling. Explicit global null disables processing timeout;
transport inactivity remains active. Timers do not preempt blocking code, undo
external effects, stop detached work, or time response streaming after headers.

The managed adapter uses the existing Hyper stack's HTTP/1+HTTP/2 builder,
validated header/read/admission/reuse controls and an independent inactivity
watchdog. Recoverable accept errors retry with interruptible backoff. Connection
peer extensions wrap the shared router once rather than rebuilding route maps.
Finalized routers retain connection settings through serving/nesting; raw Axum
embedding transfers connection-policy ownership to the embedding server.

HTTP/2 stream futures have connection-local task ownership. Grace expiry signals
force; the connection drops its driver, closes its stream scope, aborts/joins
pending stream futures, then the server joins connection owners before returning.
The 32-iteration/two-worker regression first exposed and then verified correction
of a real resource-cleanup race with detached Hyper stream futures.

Upgraded IO retains its admission permit until released. After protocol handoff,
applications own upgraded-session inactivity, frame limits and shutdown; HTTP
watchdogs do not claim to terminate detached WebSocket work. No unsupported
HTTP/2 absolute header deadline, absolute connection lifetime, per-IP policy or
TLS API is advertised. Reverse proxies own additional ingress/TLS policies.

The runner owns BootUiServer, requests drain and joins it before resource release.
Its listener uses the same configured request/connection/drain budgets (admission
is per listener). Generated startup rollback and normal resource cleanup receive
the resolved ShutdownPolicy. Synchronous hooks and scheduler callbacks cannot
be safely forced to stop; scheduler shutdown joins them. Grace is per supported
wait/contract, not a whole-process termination guarantee. Resource callbacks
receive advisory policy and must implement any deadline themselves. On serving
failure the runner stops scheduler dispatch, requests resource cancellation,
then joins consumers before shutting down their owners. This also covers bind
errors where the signal callback never ran.

## F. Security review

An independent security/correctness reviewer inspected the implementation and
follow-up diffs. Confirmed findings were evaluated and corrected:

| Finding | Correction and evidence |
| --- | --- |
| Accept error ended whole server | Recoverable connection-local retry/backoff; shutdown remains responsive |
| Failed body remained pollable | Terminal body drop/fusion; continues-after-error handler test observes exactly two underlying polls |
| No-argument compatibility wrappers disabled default deadline | Inherit default budget; explicit Option None APIs retain intentional opt-out |
| Router layers rebuilt per connection | One Tower peer Extension around shared Router |
| Standalone actuator accepted invalid protected credentials | Checked typed validation rejects before serving; legacy path still fails closed |
| BootUI malformed/unsafe bind/path configuration | Checked before nesting/listener startup with redacted errors |
| Managed BootUI outer context retained permit_all | Clear application permits at both boundaries; actual managed TCP and generated-main auth regressions |
| H2 handler survived forced connection drain | Scoped executor abort/join; 32 two-worker iterations verify resource-cleanup ordering |
| Scheduler special paths changed generic domain loading | Gate framework-specific splitting/environment registration handling; nested-domain regressions |
| Selected base file rejected on profile reapplication | Preserve selected file identity/role and extension ordering, including path aliases |
| Programmatic overrides absent from standard startup | Typed loader-taking runner APIs and generated hook; actual web/all on-wire precedence and literal example tests |
| Bind-failure cleanup deadlocked on a dependent scheduled job | Stop dispatch, cancel dependencies, join consumers; actual occupied-port regression |
| Loader Debug exposed programmatic secrets | Omit all override keys/values, including cloned/pretty/nested Properties diagnostics |

Independent follow-up reviewer `/root/reassess_http` found no remaining confirmed
blocker in the corrective changes. The reviewer personally executed
`cargo test -p scafra --features web,scheduler --locked --test startup_loader --test startup_bind_cleanup`
(5 passed), `cargo test -p scafra-config --locked --test debug_redaction`
(2 passed), and `git diff --check`. The reviewer inspected the generated script
and its recorded results, but did not personally run that script or the entire
workspace/MSRV suite. [Review record](../../target/cfg-111/final-verification/independent-review.json).

Security suites retain missing/invalid/valid credential, role/scope, hidden404,
unavailable feature, JWT/JWKS, redaction, nested/raw-router and GET/HEAD/header
contracts. Enabled incomplete Basic pairs reject even when another scheme is
usable. Cryptography, required claims, fail-closed enforcement, remote response
cap/no redirects and BootUI secret allowlisting remain immutable invariants.

## G. Performance and resource cost

No performance improvement or benchmark result is claimed. Benchmarks: NOT RUN.
Request accounting is streaming and O(1) in body size, with a request-local shared
failure flag and erased bounded body wrapper. Successful requests do not parse
configuration or serialize error JSON. Deadlines resolve once from validated
startup values. Connection admission/watchdogs/task scopes add bounded ownership
state. The H2 executor uses a short connection-local mutex for stream spawning
and reaps completed entries; there is no process-global registry or HTTP/1
per-request task-scope lock. No whole-body framework buffering or unbounded
request queue was added. HTTP/2 adds `h2`/transport dependencies to web consumers.

## H. Compatibility

Package versions remain 0.1.0; Rust MSRV remains 1.88. The proposed API policy is
not treated as a shipped 1.0 guarantee. New public ServerConfig/RouterOptions/
JwtConfig fields and ScafraConfig.shutdown require exhaustive literal updates;
`..Default::default()` is the migration path. ServerConfig's validation error
changes from Infallible to ValidationError. New checked helper/server APIs and
ShutdownConfig reexports are documented. Existing paths/serving signatures and
explicit Option-based opt-out APIs remain available.

Default timeout/connection behavior intentionally tightens. Alias duplicates,
unknown settings, invalid actuator values/Basic pairs, unsafe BootUI paths and
late selectors fail early. Lists now work in properties/environment/programmatic
leaves. Scheduler identifiers are preserved with reversible environment spelling.
Generated graphs gain policy-taking async composition while retaining existing
wrappers. BootUI gains an owned lifecycle and web dependency; web-only consumers
still exclude security/JWT/JWKS/actuator/scheduler. See [upgrading](../upgrading.md).

New loader APIs and the optional macro hook are additive. Existing runner
signatures and empty `#[scafra::main]` remain valid. Unknown macro arguments now
produce diagnostics rather than being ignored. Debug output is deliberately
redacted; it is not a serialization API. No new dependency, global state or
per-request layer was introduced by these corrections.

## I. Executed verification

Toolchain: Rust 1.99.1 (`58cf8f9c3`, LLVM 23.1.1), Cargo 1.99.1,
`x86_64-unknown-linux-gnu`. MSRV toolchain: installed Rust 1.88.0.
Workspace runs use `TMPDIR=$PWD/target/cfg-111/tmp` and `RUST_TEST_THREADS=2`.
All logs and machine-readable histories are preserved locally in ignored output.
The post-review pass records source fingerprints before and after each command.

| Exact required command | Final-source outcome | Local log |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASSED | [log](../../target/cfg-111/final-verification/fmt-final.log) |
| `cargo check --workspace` | PASSED | [log](../../target/cfg-111/final-verification/check-workspace-final.log) |
| `cargo test --workspace` | PASSED | [log](../../target/cfg-111/final-verification/test-workspace-final.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASSED | [log](../../target/cfg-111/final-verification/clippy-workspace-followup.log) |
| `cargo check --workspace --all-features` | PASSED | [log](../../target/cfg-111/final-verification/check-all-features-followup.log) |
| `cargo test --workspace --all-features` | PASSED | [log](../../target/cfg-111/final-verification/test-all-features-followup.log) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED | [log](../../target/cfg-111/final-verification/clippy-all-features-followup.log) |
| `cargo +1.88.0 check --workspace --all-features --locked` | PASSED | [log](../../target/cfg-111/final-verification/msrv-followup.log) |

Final-source command records: [final-verification/results.json](../../target/cfg-111/final-verification/results.json).
All required rows record the same source fingerprint before and after execution.
The first pass crossed the small scheduler-ordering correction; its affected
format/check/default-test commands were repeated on the final source. Later
all-feature/Clippy/MSRV rows already used that final source and were retained.

Earlier machine-readable full history: [verification/results.json](../../target/cfg-111/verification/results.json).
Earlier failed attempts are retained, including corrected old expectation/format/
Clippy failures and the real multithreaded H2 cancellation defect; they have not
been relabeled as passing. Final-source totals: default **347 passed /0 failed /2 ignored** across 76 harnesses; all features **479 passed /0 failed /2 ignored** across 89 harnesses.
Both include18resource tests; all features includes20operational configuration
tests and6scheduler-name tests. The ignored CLI loopback test passed separately;
the other ignored item is the existing actuator registration documentation
example. Nested Cargo fixture results are not inflated into these counts.

Additional executed evidence:

| Check / command | Outcome and preserved evidence |
| --- | --- |
| `cargo test -p scafra-config --all-features` and minimal variants | PASSED; [base-role full logs](../../target/cfg111-config-base-role-all-final.log), [minimal](../../target/cfg111-config-base-role-minimal-final.log), [operational](../../target/cfg111-config-base-role-operational-final.log); all6sources, lists, aliases, difficult scheduler names, domain isolation, selector phase/identity and actual2ms execution |
| `cargo test -p scafra-security --all-features`; minimal/JWT focused variants | PASSED; [latest security](../../target/cfg-111/security-final.log), [feature/runtime ledger](../../target/cfg111-operational/results.json); configured JWKS TLS connect/total/cache/cooldown and claim skew |
| `cargo test -p scafra-web --all-features --test resource_limits` and full workspace runs | PASSED focused/current suite coverage; [18-test source](../../crates/scafra-web/tests/resource_limits.rs), [32-iteration fixed test](../../target/cfg-111/h2-cancellation-fixed.log), [upgrade admission](../../target/cfg-111/upgrade-admission.log) |
| `cargo test -p scafra-bootui -p scafra-actuator` | PASSED; [focused logs](../../target/cfg-111/bootui-actuator-final.log), [managed auth/bind regressions](../../target/cfg-111/review-regressions.log) |
| `cargo test -p scafra-scheduler --locked` | PASSED; [focused log](../../target/cfg-111/scheduler-focused.log) |
| Facade configured TCP / generated resource cleanup fixtures | PASSED; [runtime](../../target/cfg111-facade-runtime-final.log), [graph fixtures](../../target/cfg111-graph-runtime.log), [BootUI runner wiring](../../target/cfg111-bootui-wiring.log) |
| CLI units/dev controls and generated project matrix | PASSED; [CLI units](../../target/cfg-111/cli-unit.log), [dev](../../target/cfg-111/cli-dev.log), full workspace logs |
| `cargo test -p scafra-cli --test generated_projects generated_projects_serve_routes_and_shutdown_gracefully -- --ignored` | PASSED explicitly; [acceptance log](../../target/cfg-111/verification/generated-cli-runtime-acceptance.log); not inferred from ignored default execution |
| `python3 scripts/issue12-http.py ... --features <web/security/jwt/actuator/standard/all>` | PASSED six independent generated-main HTTP/security/error/HEAD/SIGTERM matrices; [commands/results](../../target/cfg-111/generated-http/results.json) |
| `python3 scripts/issue111-runtime.py target/cfg-111/generated-reviewed --target-dir target/cfg-111/generated-http/shared-target` | PASSED40cases: six generated feature variants × six source modes, three generated banner/logging cases, one HTTP-free scheduler consumer; [results](../../target/cfg-111/generated-reviewed/results.json), [run](../../target/cfg-111/generated-reviewed.log) |
| `python3 target/cfg-111/host_smoke.py` | PASSED nondefault127.0.0.2 bind, other loopback address refused, SIGTERM; [result](../../target/cfg-111/host-smoke/results.json) |
| `python3 target/cfg-111/documentation_consumer.py` | PASSED literal programmatic reference compiled/executed with minimal facade and normal dependency tree excluding HTTP/security; [result](../../target/cfg-111/documentation-consumer/results.json) |
| YAML/properties reference fixtures | PASSED exact equality with executed operational fixtures; [results](../../target/cfg-111/documentation-examples.json) |
| `python3 scripts/check-api-policy.py` | PASSED; [log](../../target/cfg-111/verification/api-policy-acceptance.log) |
| `python3 -m unittest discover -s scripts -p test_api_policy.py` | PASSED16tests; [log](../../target/cfg-111/verification/api-policy-drift-acceptance.log) |
| `python3 scripts/arch103-publish-order.py --check` | PASSED; [log](../../target/cfg-111/verification/publish-order-acceptance.log) |
| `git diff --check`, documentation local-link checks, Python syntax | PASSED after report creation; no hosted CI claim |

One expanded generated test initially used a shared bearer value while JWT
verification was enabled; the framework correctly denied it. The fixture was
corrected to use its explicitly configured Basic scheme. The passing40case
matrix checks authentication rather than weakening the JWT fallback invariant.


Follow-up execution also passed:

| Check | Outcome / evidence |
| --- | --- |
| `cargo test -p scafra --no-default-features --features web --test startup_loader --locked` | PASSED; [log](../../target/cfg-111/final-verification/minimal-loader-final.log) |
| `cargo test -p scafra-cli --test generated_projects generated_projects_serve_routes_and_shutdown_gracefully -- --ignored` | PASSED; [log](../../target/cfg-111/final-verification/generated-cli-runtime-final.log) |
| `python3 scripts/check-api-policy.py` | PASSED; [log](../../target/cfg-111/final-verification/api-policy-final.log) |
| `python3 -m unittest discover -s scripts -p test_api_policy.py` | PASSED; [log](../../target/cfg-111/final-verification/api-policy-drift-final.log) |
| `python3 scripts/arch103-publish-order.py --check` | PASSED; [log](../../target/cfg-111/final-verification/publish-order-final.log) |
| `python3 scripts/issue111-startup.py target/cfg-111/followup-generated-startup-final-v2 --target-dir target/cfg-111/generated-reviewed/target` | PASSED web/all/literal-reference consumers; [3-case results](../../target/cfg-111/followup-generated-startup-final-v2/results.json) |
| `python3 scripts/issue111-runtime.py target/cfg-111/followup-generated-matrix --target-dir target/cfg-111/generated-reviewed/target` | PASSED 40 generated runtime cases; [results](../../target/cfg-111/followup-generated-matrix/results.json) |
| Literal minimal programmatic reference | PASSED compile/run and dependency closure; [results](../../target/cfg-111/followup-documentation-consumer/results.json) |

The new generated hook cases prove files/profile/environment/programmatic
precedence through actual listener, body, timeout and logging behavior, a single
hook invocation after main, BootUI effective address/profile, and SIGTERM with
closed sockets. The exact new documentation example compiles unchanged and
accepts 800 bytes while rejecting 801 bytes with the JSON 413 contract.

### External verification completed — 2026-10-25

The task owner explicitly authorized `/var/tmp/scafra-111-verification-20261024`
and descendants. Tests used that exact subtree, with a shared temporary directory
inside it and `CARGO_BUILD_JOBS=4`. Framework configuration environment controls
were removed from the test process environment so unrelated deployment settings
could not alter fixtures. No publication, remote resource mutation or Git change
was performed. All three external suites were rerun after the final Rust source correction.

| Exact external command | Outcome | Evidence |
| --- | --- | --- |
| `RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/arch103-consumers.py /var/tmp/scafra-111-verification-20261024/final-consumers-msrv --proposed --runtime --target-dir /var/tmp/scafra-111-verification-20261024/consumer-target-msrv --timeout 600` | **PASSED**16isolated consumers, all lock/tree/build/runtime checks; no forbidden dependencies;72.84s | [results](../../target/cfg-111/final-verification/consumers.json), [execution](../../target/cfg-111/final-verification/consumers-execution.json) |
| `RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/arch103-negative.py --output /var/tmp/scafra-111-verification-20261024/final-negative --target-dir /var/tmp/scafra-111-verification-20261024/negative-target --timeout 600` | **PASSED**9negative cases;48.014s | [results](../../target/cfg-111/final-verification/negative.json), [execution](../../target/cfg-111/final-verification/negative-execution.json) |
| `python3 scripts/arch103-package-check.py /var/tmp/scafra-111-verification-20261024/final-packages --offline` | **PASSED**12packaged/extracted crates and2registry-source macro consumers;125.604s | [results](../../target/cfg-111/final-verification/packages.json), [execution](../../target/cfg-111/final-verification/packages-execution.json) |

The consumer cases are minimal, config, core, foundation, web, secure-web,
inventory, jwt, nested, jwks, actuator, scheduler, default, standard, bootui and
all. Consumer and negative checks ran on Rust1.88.0 (`6b00bc388`, LLVM20.1.5),
`x86_64-unknown-linux-gnu`; package verification ran on Rust1.99.1/LLVM23.1.1.
The catalogue checks feature closure independently and exercises fail-closed
credentials/roles/scopes, nested composition, resource cleanup, readiness,
scheduler-only execution and safe BootUI snapshots where applicable.

The initial negative run had seven passing and two failing cases because two
capability-unification fixtures still used the old exhaustive RouterOptions literal (E0063). The shared
fixture now adds `..Default::default()`, preserving its explicit security and
actuator compile-time probes and explicit `request_timeout: None`. No capability
assertion was removed or weakened. The complete suite then passed all nine cases. The failed
[initial results](../../target/cfg-111/external-results/negative-initial.json) and
[execution](../../target/cfg-111/external-results/negative-execution.json) remain
preserved, alongside the successful retry. These expected negative cases include
compiler101/startup1 outcomes; the verification scripts themselves exit0 on success.

Full sources, locks, trees, archives and logs remain under the approved subtree.
Small machine-readable results are also copied into ignored
`target/cfg-111/external-results/`; [summary](../../target/cfg-111/external-results/summary.json).
The earlier external-evidence review covered the fixture migration and catalogue.
The follow-up source review and its personally executed tests are described in F;
it is not claimed to independently rerun the complete external catalogue.

There are no outstanding local verification or permission blockers. Hosted CI,
published crates.io smoke tests and remote Git consumers are not claimed by this
local package/registry-source verification. No package was published.

## J. Issue #111 acceptance criteria

All criteria below pass within the implemented and documented contract. Scope
limitations are explicit; no unsupported protocol or termination guarantee is inferred.

| Criterion | Status | Evidence / limit |
| --- | --- | --- |
| All twelve crates audited | PASS | Twelve-crate manifest/source inventory |
| All supported operational defaults inventoried | PASS |47runtime schemas +profile and30classified candidates |
| Runtime keys/types/units/bounds documented | PASS | Canonical reference and owning validation |
| Existing settings consumed | PASS | Consumer traces plus logging/security/health/scheduler/BootUI runtime suites |
| Missing operational overrides implemented | PASS |15runtime keys +CLIpoll |
| YAML/YML/properties | PASS | Resolution matrix +real generated source modes |
| Profile overrides | PASS | Matrix/on-wire; misplaced/late selectors reject, base-file identity preserves precedence |
| Environment including underscores and task names | PASS | Real process environment tests, collisions/encoding/duplicates |
| Programmatic overrides | PASS | Resolution matrix, actual generated loader hook, literal reference, authoritative facade TCP tests |
| Precedence preserved | PASS | Cross-source/extension/profile/alias regressions |
| Effective settings reach runtime | PASS | Generated main +typed composition +standalone consumers |
| Invalid/unknown/unavailable settings fail early | PASS | Config/source/schema/feature/overflow/redaction negatives |
| Compile-time/fixed exceptions justified | PASS | Source-linked candidate inventory, independent review |
| Configurable safe default body limit | PASS |1MiB default and custom ceiling |
| Raw bodies bounded | PASS | Streaming consumed-byte ceiling, no framework prebuffering |
| Streaming cannot bypass delivery ceiling | PASS | Exact/over/chunked/error-swallow/fusion/cancellation tests; unread/committed-response scope explicit |
| Appropriate413JSON | PASS | Where detected before response commit; no universal413 claim |
| Finite justified processing default | PASS |30s actualTCP deadline test |
| Timeout shorter/longer/explicit disable | PASS | RealHTTP +null resolution; outer ceiling retained |
| Correct504/tie/cancellation contract | PASS | Existing +new timeout regressions |
| Header/idle/connection ownership verified | PASS | Managed Hyper adapter and documented raw/upgrade ownership |
| Managed protections enforced | PASS | Slowheaders, inactivity, admission, headerbytes/count, H2stream tests |
| Route policy predictable | PASS | Tightening tests; no silent outer-ceiling removal |
| Shutdown configurable and consumed | PASS | HTTP/nonforcing/H2 +generated normal/rollback resource policy tests |
| Authentication/authorization fail closed | PASS | Security/JWT/JWKS/nested/raw/BootUI regression suites |
| Secrets remain redacted | PASS | Diagnostics, override Debug/Properties, error bodies, logs and exact BootUI snapshot tests |
| Router composition does not bypass limits/security | PASS | Managed/raw/nested/global ceiling regressions |
| Optional Cargo feature isolation | PASS | All16independent consumers checked on Rust1.88 with no forbidden normal dependencies |
| Rust1.88workspace compatibility | PASS | Locked all-feature workspace check plus16independent MSRV consumers and9negative cases |
| Public API migration documented | PASS | Reference/API inventory/upgrading/generated examples |
| Generated apps compile/run | PASS | CLI matrix, six generated-main feature variants, worker, explicitignoredHTTPtest |
| Existing HTTP status/header/error contract preserved | PASS | #12generated matrix +framework regressions |
| Seven mandatory workspace commands | PASS | All seven mandatory commands exit0 on the final source fingerprint |
| Relevant focused crate tests | PASS | Commands/logs above, supplemented by workspace suites |
| Default and overridden runtime behavior | PASS |40-case matrix +3 new hook/reference consumers +host/claim/cache/health/scheduler/lifecycle tests |
| Negative HTTP/resource tests | PASS |18resource tests incl32H2concurrency iterations |
| Full feature-isolated consumer catalogue | PASS | Existing script:16/16lock/tree/build/runtime cases on Rust1.88 |
| Security regression suites | PASS | Minimal/JWT/allfeature and workspace/runtime tests |
| Real generated application tests | PASS | Source variants and explicitCLI loopback/graceful test |
| Documentation examples match implementation | PASS | Literal compile/run and fixture equivalence |
| Independent source review without unresolved confirmed blocker | PASS | Source review +7 personally rerun tests; complete suite run by primary |
| Independent negative consumer fixtures | PASS | Existing script:9/9; original fixture failures retained and corrected |
| Staged package/registry consumer checks | PASS |12extracted packages and2version-only registry-source consumers |
| Complete issue/local PR readiness | PASS | All required local evidence present; hosted CI and maintainer approval separate |

## K. Final decision

**READY FOR PR REVIEW.** All issue #111 acceptance criteria above are resolved
within the implemented, tested and documented contract. The three additional
reassessment defects are corrected and independently reviewed. Seven mandatory
workspace commands, locked Rust 1.88 check, isolated MSRV consumers, negative
fixtures, generated runtime/documentation consumers and extracted package checks
pass on the updated implementation. There is no unresolved confirmed release
blocker. Advisory resource cleanup and committed/unread-stream limitations are
explicit; no unsupported enforcement guarantee is inferred. Hosted CI and
maintainer approval remain separate. No agent commit, PR, push, merge,
publication or issue edit was performed.
