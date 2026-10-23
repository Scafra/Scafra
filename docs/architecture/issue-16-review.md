# Issue #16 implementation review

Status: **READY FOR PR REVIEW** — implementation deliverables and local checks
are complete; maintainer approval and hosted CI remain separate.
Policy revision 1 is **proposed for maintainer approval**, not an assertion that
1.0.0 has shipped or is ready for production.

## Repository evidence and scope

- Required branch: `16-establish-framework-api-stability-and-compatibility-policy`.
- Starting HEAD: `986f1bb`, merge of PR #104; clean working tree before edits.
  Its parents/history include #103's modular implementation and subsequent
  route-local/managed-startup security corrections. No branch change was needed.
- Root Cargo workspace has twelve packages, shared version `0.1.0`, edition 2021,
  resolver 2 and `rust-version = "1.88"`. Facade defaults are empty; `standard`
  contains web/security/jwt/jwks/actuator/scheduler and excludes BootUI.
- Read issue #16's actual acceptance criteria and default-feature decision via
  GitHub, issue #103 and linked #9/#15/#33/#39/#42/#43. PR #104 is closed and
  appears as the current HEAD merge. #33's original 1.82 text is historical;
  the checked-in manifests and existing consumer CI now use 1.88.
- Inspected all twelve manifests, crate roots/public contracts, macro expansion
  sources and graph renderer, requested architecture/config/security/generated/
  publishing/contribution docs, ARCH-01 history/evidence, consumer/negative/package
  scripts and both workflows. Historical ARCH-01 baselines are not current API
  assertions. Existing tests were inspected before making the correction.

No versions, defaults, dependencies, lockfiles or MSRV were changed. Nothing
was committed, pushed, tagged, released, published, or sent as a GitHub comment.
No other repository was modified. Build/test artifacts remain outside Git.

## Findings and decisions

1. Public structs and exhaustive enums are real compatibility hazards, including
   configuration fields, graph descriptors and error variants. Feature selection
   changes their visible shape; adding a Serde-defaulted field is not automatically
   a compatible Rust API addition. Policy freezes these contracts after approval.
2. Public macros rely on facade/core/web hidden helpers, shared foundation types
   and build-generated graph code. `doc(hidden)` cannot remove their technical
   compatibility obligations. Keep existing compatible Cargo ranges, release
   all twelve packages together and preserve admitted macro/runtime resolutions.
   Same-full-version pairing is recommended/tested; arbitrary combinations are
   not promised. New helper use must examine and raise relevant dependency lower
   bounds or preserve old expansion contracts. No blanket exact pins were added.
3. Managed and raw router APIs have distinct supported semantics. Generated
   route-local guards survive composition; custom managed handwritten routes
   need explicit guarding. Finalized serving avoids repeated middleware.
   Protected routes/unavailable authentication intent must fail closed even
   under feature union. These are stable behavior invariants.
4. Candidate 1.0 surfaces include core lifecycle/typed graph/resources, typed
   config/validation, routing/errors, authentication/authorization, graceful
   HTTP runtime, scheduler/operations and documented macro/generated APIs.
   BootUI/extension/custom build pipelines and debug reload remain explicitly
   experimental. Supervised HTTP-free workers and unimplemented integrations
   remain future/unsupported. Candidate does not mean already stable at 0.1.0.
5. Empty defaults are retained as the proposed 1.0 decision: real capability
   opt-in with explicit web CLI starters. `standard` freezes the six existing
   capabilities through 1.x; additions need separate selection/bundle. Defaults,
   removed/renamed features or incompatible feature meaning need major releases.
6. Post-1.0 patches preserve supported contracts and fix defects; minors add
   compatible functionality; majors permit intentional source/behavior breaks.
   Possibly-breaking Rust additions need downstream coherence/inference/method
   review. Existing 0.y patches preserve that line (compatible additions allowed),
   breaks use next 0.y. Pre-release labels do not establish stable support.
7. Proposed MSRV policy preserves Rust 1.88 through 1.x; increases need a major
   (0.x: minor), evidence and migration. All published consumer/build/macro/CLI
   code is in scope; stable-only development tooling is separately identified.
   At least one compatible dependency resolution is required, not every future
   allowed third-party release. Clean and locked resolutions are distinct gates.
8. Sole officially tested target: Linux GNU x86_64,
   `x86_64-unknown-linux-gnu`. Windows/macOS have expected-to-work source paths,
   not equivalent verification. Other target terms/promotion rules are explicit.
9. A new public-API diff CI dependency was not justified for this branch. Stable
   signatures are covered by existing API/consumer compile tests plus required
   maintainer public API diff review. #39/#42 must record either a maintained
   pinned diff-tool/baseline or the manual diff artifact before 1.0; macro and
   behavior reviews remain necessary regardless of tooling.

### Focused correction: startup macro hygiene

A fresh external application depending only on `scafra` invoked
`scafra::startup!("consumer startup")`. Before correction, compilation failed
with **E0433: cannot find `tracing` in the list of imported crates**. Foundation's
macro expanded to caller-relative `::tracing::info!`, requiring an undeclared
implementation dependency from application authors.

Changed it to `$crate::info!`, using foundation's existing public re-export.
The event target and logging behavior are preserved. No dependency was added.
Added an actual startup invocation to isolated minimal/default consumers, which
have no direct tracing dependency. The same external fixture then compiled on
Rust 1.88. Before/after evidence is under
`/var/tmp/scafra-issue16-validation/startup-{before,after}.log`.

## Files created or modified

| File | Change |
| --- | --- |
| [docs/api-stability.md](../api-stability.md) | Canonical versioned proposal, source-linked inventory, Rust compatibility matrix, twelve-crate/generated-code/feature/MSRV/target/security/release policy |
| [docs/upgrading.md](../upgrading.md) | Capability/source migration and practical version/config/security/lifecycle upgrade procedure |
| [docs/contributing.md](../contributing.md) | Short compatibility review checklist, rationale/migration/tests/decision requirements and ADR threshold |
| [docs/publishing.md](../publishing.md) | Coordinated release/RC gates integrated with existing publication procedure and related issues |
| [docs/modules.md](../modules.md) | Canonical policy and upgrade links beside the implemented feature reference |
| [docs/README.md](../README.md) | Documentation navigation |
| [README.md](../../README.md) | Discoverable policy/upgrade entry points |
| [scripts/check-api-policy.py](../../scripts/check-api-policy.py) | TOML contract drift checks against facade features/defaults/forwarding, workspace members, inherited MSRV/version and first-party dependency requirements |
| [scripts/test_api_policy.py](../../scripts/test_api_policy.py) | Five tests including mutations for default/feature, compiler, package/inheritance/dependency and inventory drift |
| [.github/workflows/ci.yml](../../.github/workflows/ci.yml) | Policy/tests gate and selected consumer compiler consistency check; retains existing MSRV matrix |
| [.github/workflows/publish.yml](../../.github/workflows/publish.yml) | Policy/tests gate after version selection, before build/package/upload |
| [crates/scafra-foundation/src/lib.rs](../../crates/scafra-foundation/src/lib.rs) | startup macro hygiene correction |
| [scripts/arch103-consumers.py](../../scripts/arch103-consumers.py) | Minimal/default external regression exercising startup without direct tracing |
| [this report](issue-16-review.md) | Audit, decisions, executed verification and remaining sign-offs |

## Automated enforcement and limits

The new checker parses a deliberately marked TOML block in the canonical doc and
Cargo manifests with Python 3.11's standard `tomllib`; it does not parse Rust
source with regexes or add any package/tool dependency. It checks reviewed facts,
not version-bump correctness. Updating the contract and manifest together can
still introduce a break: the maintainer checklist/migration review is required.
The CLI has no workspace dependency entry because it is a binary; its package
still inherits the shared version/MSRV. Current first-party dependency entries
are compatible ranges, not exact pins.

The consumer `--rust` check rejects numeric compiler-policy drift. It accepts
stable too; removing all MSRV matrix entries requires workflow review. Existing
MSRV/isolated consumers, negative feature/security tests, graph/UI fixtures and
staged packages supply the technical checks; this branch does not duplicate
#33's target infrastructure or claim to implement #15/#42's real publication
verification. Local staged archive success cannot prove crates.io ownership,
index propagation or final artifact usability. No all-permutation feature or
all-version-resolution proof is claimed.

## Executed verification

Environment: Linux `x86_64-unknown-linux-gnu`, stable rustc 1.99.1
(`58cf8f9c3`, 2026-10-13), Cargo/Rust 1.88.0 installed and used explicitly.
Full test processes use `TMPDIR=/var/tmp` to avoid the host's small shared tmpfs.
Logs and machine-readable results are in `/var/tmp/scafra-issue16-validation`,
external consumers in `/var/tmp/scafra-issue16-consumers-final`, negatives in
`/var/tmp/scafra-issue16-negative-final` and package staging in
`/var/tmp/scafra-issue16-packages`. These paths are local evidence, not repository
artifacts or hosted CI claims.

| Exact command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS after correction |
| `cargo check --workspace` | PASS after correction |
| `cargo test --workspace` | PASS after correction |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS after correction |
| `cargo check --workspace --all-features` | PASS after correction |
| `cargo test --workspace --all-features` | PASS after correction |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS after correction |
| `cargo +1.88.0 check --workspace --all-features --locked` | PASS after correction |
| `python3 scripts/check-api-policy.py --rust 1.88.0` | PASS |
| `python3 -m unittest discover -s scripts -p 'test_api_policy.py'` | PASS, 5 tests |
| `RUSTUP_TOOLCHAIN=1.88.0 TMPDIR=/var/tmp python3 scripts/arch103-consumers.py /var/tmp/scafra-issue16-consumers-final --runtime --timeout 600 --target-dir /var/tmp/scafra-issue16-consumer-target` | PASS, all 15 isolated cases; package absence, real TCP/security/readiness/SIGTERM and ownership checks |
| `RUSTUP_TOOLCHAIN=1.88.0 TMPDIR=/var/tmp python3 scripts/arch103-negative.py --output /var/tmp/scafra-issue16-negative-final --target-dir /var/tmp/scafra-issue16-consumer-target --timeout 600` | PASS, all 9 cases including expected compilation/startup failures and capability unions |
| `python3 scripts/arch103-publish-order.py --check` | PASS, all 12 packages in both workflow arrays |
| `TMPDIR=/var/tmp python3 scripts/arch103-package-check.py /var/tmp/scafra-issue16-packages --offline --reuse` | PASS after correction, 12 extracted packages + web/standard registry-source macro consumers; initial fresh run also passed |
| `TMPDIR=/var/tmp cargo test -p scafra-cli --test generated_projects generated_projects_serve_routes_and_shutdown_gracefully -- --ignored --nocapture` | PASS, real HTTP and shutdown for all 4 generated kinds |
| `TMPDIR=/var/tmp cargo test --offline --locked --manifest-path examples/async-resource/Cargo.toml` | PASS, 6 tests |
| `cargo doc --workspace --all-features --no-deps` | PASS, no warnings |
| Relative documentation links and explicit heading anchors | PASS, 96 checked with a local filesystem/Markdown-heading check |
| TOML examples in new guides and contract | PASS, all TOML blocks parsed with tomllib |
| `git diff --check` | PASS |

The first checker run correctly exposed an over-strict checker assumption: the
CLI binary has no workspace dependency requirement. The checker was corrected
to validate actual first-party dependency entries and all member inheritance;
all subsequent checker/tests passed. The intentionally failing startup macro
reproduction is documented above. Neither failure is suppressed in this report.
The CLI network test is intentionally ignored by the default suite and was
explicitly executed here. Hosted CI, actual upload/index resolution and other
targets were not run.

## Maintainer sign-offs and release work

Required review decisions: approve candidate inventory and experimental
exceptions; empty defaults/frozen standard bundle; coordinated compatible-range
model and hidden-code obligations; strict major-only post-1.0 MSRV increases;
and Linux-only official target scope. These are concrete proposals, not approvals
invented by the implementation. A maintainer must accept this policy before it
becomes the release promise, and approve the final 1.0 inventory at its commit.

#9 performance evidence, #15 published generated templates, #33 ongoing compiler/
target evidence, #39 release go/no-go, #42 exact RC/upgrade/final-artifact evidence
and #43 security ownership/advisory process remain their respective release
owners' scope. Their issue closure and production readiness are not asserted.
Independent review found no material policy/code inconsistency after the MSRV
checker limitation was clarified. It also validated the startup hygiene fix
and ran the policy checker/five drift tests successfully.

## Implementation acceptance

- Canonical source-linked inventory/policy, four stability levels, Rust-specific
  matrix and current 0.x/future 1.x distinction: delivered and source-reviewed.
- All twelve crate roles, generated-code coordination, feature/default/bundle,
  MSRV/target and security compatibility decisions: explicit proposals with
  Cargo/source consistency checks and existing runtime verification.
- Upgrade guide, review/ADR checklist, publishing integration and navigation:
  delivered; relative links/anchors and new TOML examples verified.
- Automated drift safeguards and regression correction: implemented, reviewed
  and tested; required workspace, compiler, external/negative/package checks pass.
- Product-level approval, hosted CI and broader 1.0 release checklist: remain
  maintainer/release gates, not missing local implementation deliverables.

**READY FOR PR REVIEW**
