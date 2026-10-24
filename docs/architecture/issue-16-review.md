# Issue #16 implementation review

> The original implementation record below describes policy revision 1 at
> `264b5c3`. The [Independent Merge Readiness Review](#independent-merge-readiness-review)
> supersedes its policy recommendations and readiness assessment. Historical
> command results are retained as history, not reused as current verification.

Historical revision 1 status: **READY FOR PR REVIEW** — implementation deliverables and local checks
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
7. The superseded revision 1 MSRV proposal preserved Rust 1.88 through 1.x; increases need a major
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

Historical revision 1 review decisions (superseded below): approve candidate inventory and experimental
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


## Independent Merge Readiness Review

Review of commit `264b5c33bec9ae4aa79a64a25f21b105950b0a66`, 2026-10-23.
Policy revision 2 remains **proposed**, not approved. Fresh local verification is complete. Policy approval and successful hosted CI
remain separate requirements.

### A. Findings

| ID / severity | Confirmed evidence and impact | Resolution |
| --- | --- | --- |
| IR1 / medium, policy sustainability | Revision 1 required Rust 1.88 throughout 1.x without alternatives or evidence of long-term dependency/compiler maintenance capacity. This is legal as a project policy but could require source-compatible major releases or maintenance forks for dependency/security evolution. | Compare A/B/C and propose managed minor-release MSRV evolution with explicit gates. Current Rust 1.88 unchanged; approval outstanding. |
| IR2 / medium, premature contract wording | HTTP inventory called error/status behavior, body limits and timeouts contracts while #12/#19 remain open. Source uses `{code,message}` errors in AppError/JsonBody, separate security/timeout responses, a fixed extractor limit and optional processing timeout. These are not the complete pending error/protection design. | Separate implemented candidates from final approval; add exact #12/#19/#15/#33/#42 freeze gates. No implementation of those issues here. |
| IR3 / medium, ambiguous classification | Candidate aggregate fields included `ScafraConfig.bootui` but BootUiConfig was experimental elsewhere. Broad component registration wording also obscured optional extension exceptions. | Mark the full BootUI config surface/aggregate field and all re-exports experimental; name candidate vs experimental registrations explicitly. |
| IR4 / medium, checker false negatives | With current inputs, deleting workspace.dependencies.scafra-foundation returned `[]`. Renaming that entry/member edges to foundation-alias with package=scafra-foundation and wrong version also returned `[]`. Swapping canonical package identities could also pass. The checker validated existing name keys, not missing inherited edges or effective package aliases/identities. | Resolve aliases and check inherited edge presence; add deletion/alias/target-build regressions. Cargo would reject some invalid inputs too, but the advertised pre-build drift guard must not silently accept them. |
| IR5 / low, checker input diagnostics | Contract field types were unchecked; a scalar feature forwarding value could trigger an unhandled TypeError, and a final valid TOML block could be accepted without a closing fence. | Validate small contract schema/fence and test invalid syntax/types via CLI, exit 1 with no traceback. |
| IR6 / low, overly literal compatibility rules | Any default array change and removal of a feature dependency were described as major regardless of unchanged effective capabilities or public types. | Preserve empty defaults/effective standard opt-in boundary; allow equivalent forwarding and compatible private dependency maintenance. Real capability/source/behavior breaks still require a major. |
| IR7 / low, verification gap | Startup hygiene fix was sound, but its consumer regression only compiled simple facade calls; no direct-foundation/structured-event assertion existed. | Extend actual isolated facade/default fixtures and add direct-foundation case to CI; assert target and structured fields on Rust 1.88. |

No new authentication, resource ownership or runtime correctness defect was
confirmed. Production Rust code in `264b5c3` remains unchanged by this review.
The startup fix uses `$crate` to resolve foundation's existing info re-export,
which then expands through tracing's own hygienic macro paths.

The suspected snapshot issue was **withdrawn after reproduction**: changing
only workspace.package.version did not simulate the actual workflow, whose
global sed rewrites matching dependency strings too. The real selection shell
passed for stable, snapshot and RC inputs in isolated copies. No publishing
step or credential access was performed.

### B. Corrections

| File | Focused change |
| --- | --- |
| [api-stability.md](../api-stability.md) | Revision 2: justified MSRV proposal, freeze dependencies, explicit classifications, effective feature boundaries, precise supported/tested resolutions and checker limitations. |
| [adr-002-compatibility-policy.md](adr-002-compatibility-policy.md) | Proposed decision with alternatives/consequences, as required by the compatibility review checklist for policy changes. |
| [upgrading.md](../upgrading.md) | Compiler transition, experimental BootUI aggregate construction, unfinished HTTP contract guidance. |
| [contributing.md](../contributing.md) | Review declared compiler and any justified/notified minor-release floor change. |
| [publishing.md](../publishing.md) | HTTP freeze gates, compiler decision, coordinated lower-bound vs exact pin and tested snapshot rewrite. |
| [check-api-policy.py](../../scripts/check-api-policy.py) | Contract validation, alias-aware versions and missing inherited dependency detection. |
| [test_api_policy.py](../../scripts/test_api_policy.py) | Sixteen tests: original mutations plus malformed contract/CLI behavior, closing fence, missing/aliased/canonical-identity/target edges, valid release versions, actual selection shell, and deliberate Rust API deletion demonstrating limits. |
| [arch103-consumers.py](../../scripts/arch103-consumers.py) | Foundation case; assert startup target/structured fields in facade/default/direct foundation consumers with no direct tracing dependency. |
| [ci.yml](../../.github/workflows/ci.yml) | Include foundation in existing Rust 1.88 matrix; keep all previous cases. |
| [this report](issue-16-review.md) | Separate evidence-based independent assessment and remaining decisions. |

The existing docs/README.md edit and untracked AGENTS.md, .agents/, .codex/ and
docs/development/ were present before this review and preserved. Branch and
HEAD remain unchanged. Remote main was read with git ls-remote; its current
`986f1bb5570a7f73eeeb32654692114625363949` equals the local merge base.
The full original 14-file commit diff, requested documentation, complete issue
#16, related #12/#19/#15/#33/#42, manifests, macro/generated contracts and both
workflows were inspected. Historical ARCH-01 evidence was not treated as a
fresh hosted CI result. The independent reviewer confirmed IR3/IR4 and the
snapshot correction; a second review of these corrections is recorded below.

### C. MSRV policy decision

A (strict 1.88 for all 1.x) offers maximum predictability but potentially years
of dependency/compiler maintenance. B (reviewed minor evolution) preserves
patch floors while allowing justified security/Tokio/Axum/transitive/source
needs; users taking minors review compiler changes. C (for example four stable
compiler releases) has a defined moving window but imposes recurring floor/CI
work without evidence that Scafra needs that cadence.

Recommend **B**. It matches the distinction in the official
[Cargo rust-version guidance](https://doc.rust-lang.org/cargo/reference/rust-version.html)
between compiler policy and source compatibility. Do not silently change a
patch's required compiler. A future minor needs a concrete rationale,
maintainer decision, old/new floor and affected-feature notes before final
release/RC, updated shared manifests/policy/CI and locked/fresh consumer checks
on the floor and stable. A source/behavior break still needs a major.
Pre-1.0 floor increases use the next 0.y. No current compiler increase, invented
support window, staffing, LTS or backport guarantee is introduced.

### D. API stability inventory

Stable candidates remain core lifecycle/typed graph/resources, loader/config,
HTTP/router/security, scheduler/operations and documented macros/generated
Graph/Resources/Overrides APIs. They are subject to final 1.0 inventory approval,
not frozen by merging issue #16. BootUiConfig including fields/schema, all
re-exports and ScafraConfig.bootui are experimental; enabling BootUI accepts
source effects on aggregate construction. Non-BootUI fields keep their own
classification. OptionalExtensionRegistration, ConfiguredExtensionRegistration,
custom build extension/render interfaces and user-facing reload behavior are
experimental. Hidden macro/support/supervisor plumbing is internal but must
continue to interoperate with supported generated code. Generated public graph
APIs are not dismissed as internal merely because they have generated names.
Worker/runtime reflection/database adapters not implemented remain unsupported.
No classification weakens authentication, authorization, redaction, dependency
security, resource safety or cleanup.

The SemVer matrix retains Rust-specific exhaustive field/variant, trait method,
bounds/associated-type, inference/coherence/method-resolution, re-export and
macro expansion hazards. Compatible private changes and equivalent feature
forwarding are no longer categorically major. Defaults remain empty and the
six effective standard capabilities remain the opt-in boundary; adding a new
runtime integration there is different from private helper maintenance.

### E. Open release dependencies

- **#12:** finalize typed HTTP error conversions, status/payload/schema and
  redaction across error producers; revisit AppError/WebError/ServerError and
  JsonBody rejection contracts.
- **#19:** finalize configurable/raw-body limits, processing timeout defaults,
  connection/header ownership, supported overrides and #12-aligned errors;
  revisit ServerConfig/RouterOptions/serving signatures before freeze.
- **#15:** verify every generated template/command against actual published
  packages; source/staged checks do not replace this.
- **#33:** approve compiler/target coverage and fresh resolution evidence; its
  original 1.82 issue text is historical, not the current declared floor.
- **#42:** verify exact RC/final artifacts and a representative upgrade, then
  freeze the final inventory/config/features/behavior with maintainer approval.

These remain release responsibilities, not implemented/closed here. Security
and ownership invariants hold throughout. Scafra 1.0 readiness also remains
subject to #39 and the existing security/performance release gates.

### F. Cargo compatibility

All twelve packages currently inherit 0.1.0/Rust 1.88. Internal manifest ranges
are ordinary caret-compatible lower bounds, not exact pins. Same-full-version
sets are the recommendation and currently tested source/staged distribution.
Other versions that satisfy the released manifests within a compatibility line
must still preserve supported APIs/expansions: “untested” is no exemption from
SemVer. No arbitrary cross-line, ignored-requirement or duplicate-type-identity
combination is guaranteed. No historical mixed-version package set was tested.

Macros depend on build/foundation, build on core/foundation, and emitted code
uses facade/core/web hidden paths. A new helper needs an actual dependency edge
that forces its minimum or an expansion that still works with old admitted
runtimes. Raising the facade's macro minimum alone cannot constrain an old
facade chosen by another application; macros/build have no web dependency that
would force a web minimum. Preserve that expansion contract or move the
coordinated compatibility line if no verified bound/fallback can enforce it.
A compatible addition can use a minor; an unpreservable generated/source break
needs a major (0.x: next minor), not blanket exact pins. Future lower-bound or
helper changes require actual mixed lower-bound/latest consumers.

The checker enforces the current project convention that first-party requirement
strings use the coordinated release as their simple lower bound. This is a
release consistency rule, not a general semver range solver. Existing consumer
absence/union/security suites and publish DAG are retained. No blanket pins,
new dependencies, default capabilities or architecture changes were introduced.

CI runs stable workspace checks and a separate Rust 1.88 consumer matrix; these
jobs are not dependencies inside the publish workflow. Publishing runs version
selection → policy/tests → workspace/format/Clippy/tests → DAG/staged packages
→ package lists → uploads/index waits. Release approval must require successful
CI for the exact commit; this review does not claim automatic cross-workflow
MSRV gating. Snapshot rewriting and stable/prerelease lower bounds were tested
using the exact non-publishing selection step. Local source/staged evidence
never proves crates.io acceptance or propagation.

### G. Policy checker

Verified scope: documented MSRV/member set/feature names/defaults/forwarding,
workspace version/MSRV inheritance, aliased first-party version strings,
normal/build/dev/target inherited edges, missing inherited dependency entries,
invalid contract syntax/shapes/fence and successful coordinated release rewrites.
Canonical dependency aliases must resolve to their own packages; renamed aliases
remain supported. Two- and three-component numeric compiler floors are handled.
Invalid CLI inputs fail with exit 1 and a diagnostic rather than a traceback.
A fixture replacing Rust public source with an empty file still passes, by
design: this is **not a SemVer/public API analyzer**. It cannot verify behavior,
security, macro expansions, arbitrary Cargo ranges/resolutions or compiler/target
matrix coverage. Removing all MSRV jobs still requires workflow review.

Retain manual public-signature/behavior/macro review. Before 1.0, #39/#42 should
pin and evaluate a maintained API diff/semver tool against the first real
baseline (minimal and relevant features/direct crates), or attach a manual API
diff artifact. A rustdoc-JSON tool brings compiler/nightly/version maintenance
cost; no new CI tool is justified just to claim complete compatibility checking.

### H. Tests

Fresh review commands/results are recorded here separately from revision 1.
Logs are under target/issue16-independent-review; external fixtures use the
explicitly authorized /var/tmp/scafra-issue16-independent-* prefix.
Workspace suites use TMPDIR under target/issue16-independent-review/tmp to
preserve the outside-write boundary before external-fixture approval arrived.

Environment: rustc 1.99.1 (`58cf8f9c3`, 2026-10-13), Cargo/Rust 1.88.0,
host `x86_64-unknown-linux-gnu`. Every command below returned **exit 0 / PASSED**;
expected failing fixture compilations/startups are asserted by their harnesses,
not mistaken for successful raw compilations.

| Command | Exit / observed result |
| --- | --- |
| `cargo fmt --all -- --check` | 0 / PASSED |
| `cargo check --workspace` | 0 / PASSED |
| `cargo test --workspace` | 0 / PASSED |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 / PASSED |
| `cargo check --workspace --all-features` | 0 / PASSED |
| `cargo test --workspace --all-features` | 0 / PASSED |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / PASSED |
| `cargo +1.88.0 check --workspace --all-features --locked` | 0 / PASSED |
| `cargo check --workspace --all-features --locked` | 0 / PASSED |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 0 / PASSED |
| `python3 scripts/check-api-policy.py` | 0 / PASSED |
| `python3 scripts/check-api-policy.py --rust 1.88.0` | 0 / PASSED |
| `python3 -m unittest discover -s scripts -p 'test_api_policy.py'` | 0 / PASSED, 16 tests; includes isolated stable/snapshot/RC workflow selection |
| `RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/arch103-consumers.py /var/tmp/scafra-issue16-independent-consumers --runtime --timeout 600 --target-dir /var/tmp/scafra-issue16-independent-consumer-target` | 0 / PASSED, all 16 isolated consumers, no forbidden packages; real startup/security/readiness/SIGTERM/cleanup. Facade/default/direct foundation logging assertions passed. |
| `RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/arch103-negative.py --output /var/tmp/scafra-issue16-independent-negative --target-dir /var/tmp/scafra-issue16-independent-consumer-target --timeout 600` | 0 / PASSED, 9 cases including expected compiler/startup failures and feature union |
| `python3 scripts/arch103-publish-order.py --check` | 0 / PASSED, both arrays/all 12 packages |
| `python3 scripts/arch103-package-check.py /var/tmp/scafra-issue16-independent-packages --offline` | 0 / PASSED, 12 extracted archives + 2 registry-source macro consumers |
| `cargo test -p scafra-cli --test generated_projects generated_projects_serve_routes_and_shutdown_gracefully -- --ignored --nocapture` | 0 / PASSED, all 4 generated kinds over HTTP and shutdown |
| `cargo test --offline --locked --manifest-path examples/async-resource/Cargo.toml` | 0 / PASSED, 6 tests |
| `cargo doc --workspace --all-features --no-deps` | 0 / PASSED |
| Local Markdown links/heading anchors + TOML example parser | 0 / PASSED, 112 relative links/anchors and both new-guide TOML blocks |
| `git diff --check` | 0 / PASSED |

Additional actual CLI version-emission fixtures copied normalized CLI/foundation
manifests and source into authorized disposable roots, coordinated those two
versions, and compiled/executed each on Rust 1.88 with no Scafra facade sibling
(to force registry-mode generation):

```bash
cargo +1.88.0 run --offline --manifest-path /var/tmp/scafra-issue16-independent-cli-versions/stable/cli/Cargo.toml --target-dir /var/tmp/scafra-issue16-independent-cli-target -- new /var/tmp/scafra-issue16-independent-cli-versions/stable/application
cargo +1.88.0 run --offline --manifest-path /var/tmp/scafra-issue16-independent-cli-versions/snapshot/cli/Cargo.toml --target-dir /var/tmp/scafra-issue16-independent-cli-target -- new /var/tmp/scafra-issue16-independent-cli-versions/snapshot/application
```

Both returned **0 / PASSED**. Parsed generated dependencies were respectively
`scafra.version = "0.1.0"` and `"0.1.1-snapshot.16"`, with web selected, defaults
false and no path dependency. Matching foundation fixture requirements were
rewritten too. This verifies version emission, not full prerelease publication
or downloading a generated app's dependencies from crates.io. Results are in
`/var/tmp/scafra-issue16-independent-cli-versions/results.json` and summarized in
target/issue16-independent-review/observed-results.json. No source workspace
version or lockfile changed.

The final independent reviewer reran the checker, Rust 1.88 mode, all 16 Python
tests and whitespace check successfully, and found no remaining material issue
in the reviewed corrections. All original changed files were reviewed. No
required local command is omitted. Hosted CI, real crates.io upload/index and
other target verification are **NOT RUN**; no approval or 1.0 release evidence
is inferred from local staging. The source workspace suites include the macro
UI and graph compile-pass/fail fixtures. The normally ignored CLI network test
was explicitly executed above.

### I. Approval requirements

Maintainers must explicitly approve:

1. Managed minor-release MSRV evolution and its review/notice/verification gates
   (ADR-002); the current declared floor remains 1.88.
2. Candidate vs final-freeze boundary and the named #12/#19/#15/#33/#42 gates.
3. Experimental BootUI aggregate/re-export and extension/build/reload exceptions.
4. Empty defaults and effective standard capability boundary, while permitting
   compatible forwarding/private dependency maintenance.
5. Coordinated releases with compatible ranges and generated-code obligations,
   plus Linux GNU x86_64 as the sole officially tested target scope.
6. The final 1.0 API inventory at its release commit after the release blockers.

Approval of test directories authorized only local testing, not product policy,
a commit, publication, issue update or release. None of those actions occurred.

### J. Merge decision

**READY FOR PR REVIEW**.

- **Technical correctness:** confirmed checker/policy issues corrected; all
  required local checks and targeted external/version regressions passed.
  Independent correction review found no remaining material issue.
- **Policy approval:** NOT APPROVED in this review; revision 2 and ADR-002 are
  concrete recommendations requiring the decisions in section I.
- **Hosted CI:** NOT RUN/NOT ASSERTED here; require successful required checks
  on the final PR commit, including the expanded MSRV consumer matrix.
- **Merge:** conditional on explicit policy approval, successful hosted CI and
  normal maintainer review. Local success does not authorize merge.
- **Scafra 1.0.0:** NOT READY on this review alone. #12/#19/#15/#33/#42 and the
  overall release gates still determine final API freeze/artifact readiness.

No commit, branch switch, push, PR, merge, issue mutation, tag, package upload
or release was performed. Existing unrelated work is preserved.
