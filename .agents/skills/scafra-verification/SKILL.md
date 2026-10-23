---
name: scafra-verification
description: Select and execute Scafra CI and targeted checks after framework code, macros, public API, feature, test or build-behavior changes.
---

Read AGENTS.md and reinspect .github/workflows/ci.yml before selecting checks.
For framework implementation/build changes the current workspace baseline is:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/check-api-policy.py
python3 -m unittest discover -s scripts -p 'test_api_policy.py'
```

Add checks based on the affected contracts:

- Macros/graph: `cargo test -p scafra-macros --test compile --locked` and
  `cargo test -p scafra-build --test graph_compile_fail --test graph_compile_pass --locked`;
  include relevant facade integration/generated-application consumers.
- Security/routing/config: select existing security/web/facade/config tests,
  covering denied and successful requests, nested/static/parameter/GET/HEAD
  policies, redaction and negative configuration as affected. Lifecycle changes
  need initialization failure, cancellation, rollback and shutdown tests.
- Features/public APIs/dependencies/MSRV: inspect scripts/arch103-consumers.py
  and CI's case matrix; run affected isolated cases (all cases when changing
  shared feature boundaries). CI uses Rust 1.88.0 for minimal, default, config,
  core, web, secure-web, jwt, jwks, actuator, scheduler, bootui, standard, all,
  nested, inventory, plus stable/minimal. Workspace all-features can mask leaks.
  Use the selected installed toolchain (for example `RUSTUP_TOOLCHAIN=1.88.0`),
  `cargo fetch --locked` if needed, then
  `python3 scripts/arch103-consumers.py <new-output> --proposed --case <case> --runtime`.
  Run `python3 scripts/arch103-negative.py --output <new-output> --target-dir <target>`
  for feature/config boundary changes and
  `python3 scripts/arch103-publish-order.py --check` for dependency/package changes.
  These consumers resolve offline from cached metadata; record cache/toolchain
  blockers. These harnesses reject output/target paths inside the checkout.
  Obtain explicit authorization for narrowly scoped disposable outside-checkout
  paths (and any sandbox grant) before running them, unless already authorized;
  otherwise report BLOCKED. Existing guide examples do not authorize outside writes.
- CLI/templates: include relevant generated-project compile/runtime tests.
  Release/package work follows docs/publishing.md without authorizing publication.
- Public contracts: include representative external consumers where feasible;
  record source revision, dependency source, compiler, features and lock/resolution.
  Source workspace/path tests do not validate published or pinned Git dependencies.

For trivial documentation changes, run link/content/whitespace checks; do not
automatically run Cargo. Build-instruction changes require checking the affected
commands. Record command, scope and outcome as PASSED, FAILED, BLOCKED (external
prerequisite), or NOT RUN (reason). Disclose every omitted required check; partial
validation is not success. Never invent checks, tests or benchmark results.
