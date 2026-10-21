# Reproducible current-architecture baseline

> Historical Phase 0 snapshot/planning evidence. The [implemented contract](../../modules.md)
> and [implementation results](implementation.md) supersede proposed/current-state claims below.

Measured on 2026-10-21 at `e041fc27c46e521e848ecbd3895d85b876c9b831`.
This is a **before** baseline, with no runtime/manifests refactored. The proposed
minimal capability selections cannot be measured yet: the current crates have
no features. No reduction or performance improvement has been established.

## Environment and lock state

- Branch: `103-arch-01-make-scafra-framework-modules-independently-opt-in-before-100`;
  initial working tree clean. During collection only Phase 0 scripts/docs were
  untracked; no runtime code changed.
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, LLVM 23.1.1;
  Cargo `1.99.1 (5f94df478 2026-08-27)`.
- Host/target: `x86_64-unknown-linux-gnu`, Linux
  `7.2.9-1-cachyos`, glibc 2.44.
- CPU: Intel Core i7-11370H @ 3.30GHz, 4 physical cores / 8 logical CPUs;
  `/proc/meminfo` reported 40,749,148 kB MemTotal. Ordinary developer machine,
  other processes active; CPU frequency/cache state not controlled.
- Workspace Cargo.lock is tracked and unchanged, SHA-256
  `676eefd590fb0a0d700bfeb5a0cec3fe370747eb11db08e9cb6fcb5a5ce244b3`.
- External generated consumer lock SHA-256
  `65529a781c2f80385cdb2566da6f9f1eb85f3dd080b70c7e72a54e2dc7ffbd0c`;
  archived as [baseline-consumer-lock.toml](baseline-consumer-lock.toml).
  This is Cargo.lock content stored under an explicit fixture filename, not a
  package manifest. The consumer was seeded from workspace lock and resolved
  offline before the timed builds; Cargo generate-lockfile resolves available
  cached versions. Use the archived lock to reproduce exact resolution.
- Dev profile: Cargo defaults (unoptimized, debug info, incremental).
  Release: CLI template `lto = "thin"`, `codegen-units = 1`, otherwise defaults;
  binary is unstripped. No cross-profile size/time comparison is made.

Machine-readable observations are in [baseline-results.json](baseline-results.json),
including exact commands, statuses, times, lock hashes and consumer manifest.
Full metadata/trees/logs and raw cpuinfo/meminfo remain outside the checkout at
`/var/tmp/scafra-103-baseline-final`; they are reproducible output, not build
artifacts to add to Git. An initial collector run built successfully but failed
to locate the CLI-normalized underscore binary name; it was corrected and a
fresh full run produced the results below. Initial timings are not mixed in.

## Dependency observations

Selected-host `cargo tree -e normal --prefix none` package identities are
deduplicated by name/version; root is excluded. This is normal-edge closure,
including normal dependencies of proc macros running on the host; it is **not**
all compiler work, runtime-loaded code, or linked code. Build and dev edges are
not in this count. `cargo metadata` records every declared kind/target separately.

| Crate | Direct normal declarations | Full normal closure excluding root | Indirect normal only (closure minus direct) |
| --- | ---: | ---: | ---: |
| scafra | 13 | 136 | 123 |
| scafra-actuator | 5 | 53 | 48 |
| scafra-bootui | 8 | 133 | 125 |
| scafra-build | 4 | 30 | 26 |
| scafra-cli | 5 (4 active on Linux) | 45 | 41 (active) |
| scafra-config | 8 | 131 | 123 |
| scafra-core | 3 | 29 | 26 |
| scafra-foundation | 4 | 27 | 23 |
| scafra-macros | 5 | 31 | 26 |
| scafra-scheduler | 5 | 24 | 19 |
| scafra-security | 8 | 111 | 103 |
| scafra-web | 12 | 126 | 114 |
| Generated API | 2 (scafra + serde) | 137 | 135 |

All twelve crates pass selected-package no-default checks. Twelve external
scenario consumers also compile using today's actual API. The no-default facade
still includes web, security, actuator, scheduler, JWT and reqwest; config-only
still includes the latter integrations and Axum. Direct core has none of those
packages. Current-mode secure/JWT/actuator/standard scenario labels do **not**
mean current Cargo feature selections exist; all facade closures are the same.
The future `--proposed` mode is expected to reject today's missing features and
unwanted closures, and must not be described as a passing future matrix.

## Generated API build and runtime observations

The actual CLI generated `--kind api` source, including main, service,
controller and JSON model; this is not an empty executable that can link away
all framework use. Output project declares its own `[workspace]` and has normal
dependencies on local scafra and serde. Cargo manifest path and package name
are recorded. Use the generator at the checked-out commit for future comparison.

| Operation | Observed wall time |
| --- | ---: |
| Clean dev build, fresh dedicated consumer target | 22.312693 s |
| Warm no-op build | 0.073891 s |
| Incremental dev build after appending a comment to main.rs | 0.451492 s |
| Release build, no prior release target | 36.457146 s |

Release binary: **6,881,760 bytes** (about 6.56 MiB), unstripped. Dev clean means
an empty application target; registry sources and OS caches were warm and no
download time is included. Inventory/CLI builds use a different target. These
are single observations on a shared developer host; do not compare them to
another machine, toolchain, profile, filesystem or lock resolution. Appending a
comment measures rebuild behavior, not a representative framework source edit.

The same release executable was run three times with different synthetic
configuration, loopback ephemeral ports, debug supervisor disabled, and no
inherited SCAFRA settings. A successful JSON request to
`/api/greetings/ARCH103` determined availability. Secure scenarios send the fixed
test bearer token. Polling interval is 5 ms, request timeout 200 ms, overall
deadline 15 s. This measures process-spawn to first successful HTTP response,
not an instrumented internal startup phase.

| Scenario / current compile-time closure | Spawn to successful HTTP | VmRSS and VmHWM at sample | SIGTERM exit |
| --- | ---: | ---: | ---: |
| Default config, integrations runtime-disabled | 30.164 ms | 6,432 kB | 0 |
| Security enabled, static bearer | 6.506 ms | 6,352 kB | 0 |
| Security plus actuator health endpoint selected | 6.430 ms | 6,284 kB | 0 |

Samples are from `/proc/PID/status` after the successful request. They are not
steady-state memory, allocator accounting, RSS attributable to Scafra, or a
statistical distribution. Warm caches, startup polling and host load explain
why no relative performance conclusion follows from these rows. Actuator config
is enabled in the last scenario; the baseline's HTTP readiness request still
uses the API sample route. The security/health correctness suite is separate.

Unavailable baselines: true web-only/no-JWT/no-actuator facade builds, generic
config without integrations, HTTP-free facade scheduler application, and full
supervised worker startup. Features/boundaries are absent; worker runner belongs
to #49. Direct core/scheduler normal graphs are measurable today, but they are
not substitutes for a future worker runner comparison. No cold disk-cache,
cross-target/MSRV matrix, throughput, long-lived memory or JWT/JWKS network startup
benchmark was attempted.

## Commands and reproduction

Python 3.11+ and the declared Rust toolchain are required. Populate the Cargo
cache first (`cargo fetch --locked`); offline collection must fail visibly if
sources are absent. Use **new** directories outside the checkout. No script
calls cargo clean, deletes existing targets or edits workspace source/locks.

```bash
# Inventory, exact trees and selected-package independent checks:
python3 scripts/arch103-baseline.py /var/tmp/arch103-inventory-NEW

# Full generated application measurement, exact archived consumer resolution:
python3 scripts/arch103-baseline.py /var/tmp/arch103-baseline-NEW --measure \
  --consumer-lock docs/architecture/arch-01/baseline-consumer-lock.toml

# To establish a NEW resolution instead, omit --consumer-lock.
# That was the command used for the recorded initial baseline:
python3 scripts/arch103-baseline.py /var/tmp/scafra-103-baseline-final --measure

# Isolated current consumers; unwanted deps are observations in current mode:
python3 scripts/arch103-consumers.py /var/tmp/arch103-consumers-NEW
# Future gate; expected failure at Phase 0:
python3 scripts/arch103-consumers.py /var/tmp/arch103-proposed-NEW --proposed
```

For manual inspection, commands recorded by the tool include:

```bash
cargo metadata --format-version 1 --locked --offline
cargo tree -p scafra -e normal --locked --offline --target x86_64-unknown-linux-gnu
cargo tree -p scafra -e features --locked --offline --target x86_64-unknown-linux-gnu
cargo tree -p scafra-config -e normal --locked --offline --target x86_64-unknown-linux-gnu
cargo tree -p scafra-web -e normal --locked --offline --target x86_64-unknown-linux-gnu
cargo tree -p scafra-security -e normal --locked --offline --target x86_64-unknown-linux-gnu
```

Each output directory retains metadata, normal/feature trees, manifests/locks,
command logs, targets and JSON results. Preserve the consumer lock and conditions
for subsequent before/after measurements. Source paths naturally differ across
machines; package graph/profile/toolchain equivalence matters for comparison.

## Verification and downstream limits

| Exact command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo check --workspace` | Pass |
| `cargo test --workspace` | Pass: visible harness summaries total 343 passed, 2 ignored; includes #41/#73/#74/#75 and #78 regressions |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo test -p scafra --test properties_environment` | Pass: 22 tests |
| `python3 scripts/arch103-baseline.py /var/tmp/scafra-103-baseline-final --measure` | Pass: inventory, 12 crate selections, generated builds, runtime/SIGTERM |
| `python3 scripts/arch103-consumers.py /var/tmp/scafra-103-consumers` | Pass: 12 current API scenarios; unwanted dependencies recorded |
| `python3 scripts/arch103-consumers.py /var/tmp/scafra-103-proposed --proposed` | Expected exit 1: proposed features absent and minimal/default/config closures contain forbidden dependencies |
| `cargo test -p scafra-cli --test generated_projects generated_projects_serve_routes_and_shutdown_gracefully -- --ignored --exact` | Pass: runtime smoke for all four generated application kinds, representative routes and graceful shutdown |

Ignored workspace tests are the CLI all-kinds loopback runtime smoke and one
actuator registration doctest. The runtime smoke passed when run explicitly
after measurement. The ignored doctest is not counted as executed. Python
scripts parse, JSON is valid, all local documentation links resolve, and
`git diff --check` passes. Archived-lock resolution was also checked unchanged
in a fresh generated-api project with `cargo metadata --locked --offline`.

The separate Scafra-test-services checkout was inspected read-only; its tracked
manifest/src/tests were copied to `/var/tmp/scafra-103-downstream`. Cargo tests
in that copy patched its two Git dependencies through command-line configuration:

```bash
# From the temporary copy, with CHECKOUT set to this framework checkout:
cargo --config "patch.\"https://github.com/Scafra/Scafra\".scafra.path=\"$CHECKOUT/crates/scafra\"" \
  --config "patch.\"https://github.com/Scafra/Scafra\".scafra-bootui.path=\"$CHECKOUT/crates/scafra-bootui\"" \
  test --offline
```

Result: **failed**, `tests/features.rs:103` expected 200 from BootUI but got
403. Each lib/bin unit test passed (one each). The copied test makes a direct
Tower request with no `ConnectInfo<SocketAddr>` while BootUI local-only policy
requires a loopback peer (`scafra-bootui/src/binding.rs`). This explains the
observed denial; do not weaken the guard to satisfy the old consumer assertion.
The original checkout was not edited. A separate patched `cargo test --offline
--test http` in the copy passed all **3** HTTP tests. This is evidence of a
downstream test mismatch, not a green downstream suite or a Phase 0 regression.
