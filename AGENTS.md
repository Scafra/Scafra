# Scafra engineering instructions

Inspect the relevant source, tests, manifests and CI before editing. Local code
and executable evidence take precedence over historical plans. Preserve existing
instructions and unrelated local edits. This workspace is currently 0.1.0 with
Rust 1.88 MSRV; [API policy](docs/api-stability.md) is proposed, not a shipped
1.0 guarantee. Follow [contributing](docs/contributing.md) and existing conventions.

## Engineering principles

- Prefer compile-time correctness, explicit Rust types, and sound ownership,
  borrowing and lifetimes. Use `Result`/`Option` appropriately; handle meaningful
  failures, including failure paths. Avoid production `unwrap`/`expect` where
  errors should propagate or be handled. Preserve required determinism.
- Preserve workspace/crate boundaries and feature isolation. Choose the smallest
  solution to a demonstrated problem. Prefer explicit dependency injection and
  compile-time composition; avoid service locators, runtime reflection/dynamic
  discovery, unnecessary layers, and Java/Spring-style abstractions without a
  concrete Rust benefit. Preserve Axum, Tokio, Tower, Serde and tracing interop.
- Use descriptive names, cohesive functions, straightforward control flow and
  existing formatting/naming. Avoid needless generics, traits, macros and
  indirection. Comments explain intent, invariants or non-obvious trade-offs.
  Do not shorten readable code merely to reduce lines. Unsafe is exceptional:
  justify and document its safety invariants.
- Avoid unnecessary cloning, allocation, locking, dynamic dispatch and global
  state. Keep lock scope narrow and hot paths simple; never block Tokio executor
  threads. Preserve zero-cost abstractions where practical. Performance claims
  require measurements; correctness, security and maintainability take priority.
- Authentication and authorization are security boundaries. Fail closed where
  required; never weaken security to pass tests. Protect credentials, tokens,
  configuration secrets and sensitive diagnostics. Preserve route-policy
  composition, static/parameter matching and GET/HEAD semantics. Assess limits,
  timeouts, cancellation, concurrency, resource ownership, exhaustion and errors.
- Keep public and generated APIs, supported features, MSRV and existing consumers
  in scope. Do not turn historical proposals into assumed implementation facts.

## Risk-proportionate workflow and mandatory skills

Establish the problem and evidence, affected crates/contracts and simplest sound
solution; implement a focused change, add relevant regression tests, verify,
review, correct confirmed findings, then report evidence and remaining risks.
Small changes need no planning document. Read applicable skills in
`.agents/skills/<name>/SKILL.md`; triggers combine when the change spans concerns:

| Change | Required skill |
| --- | --- |
| Substantial architecture, runtime or application composition | `scafra-design-review` before implementation |
| Substantive Rust implementation | `scafra-rust-review` after implementation |
| Auth, authorization, route security, secrets, configuration security or untrusted input | `scafra-security-review` |
| Hot paths, optimization or performance claims | `scafra-performance-review` |
| Procedural macros, typed graph generation or generated application code | `scafra-macro-diagnostics` |
| Public types/traits/macros/signatures, features, dependencies or generated APIs | `scafra-api-compatibility` |
| Framework code, macros, APIs, features, tests or build behavior | `scafra-verification` |

For material implementation changes, delegate an independent review to
`scafra-independent-reviewer`. For security-sensitive implementation changes,
delegate to `scafra-security-reviewer`; for material performance-sensitive changes,
use `scafra-performance-reviewer`. A specialist can cover general correctness
too; do not launch all agents automatically. Primary agent evaluates findings
and fixes confirmed issues. If delegation is unavailable, report that limitation
and perform a separate review pass without claiming independence.
Documentation typos and trivial edits need only relevant lightweight checks;
build-instruction changes require verification of the affected commands.

## Operational boundaries

Never commit, push, merge, publish crates, modify GitHub issues or change remote
resources without explicit user authorization. Do not make unrelated changes,
overwrite unrelated edits, modify files outside this repository without explicit
approval, or run destructive commands without explicit authorization. Do not
access secrets. Never claim compilation, tests, benchmarks or security checks
passed unless executed and observed; mark checks PASSED, FAILED, BLOCKED or NOT RUN.
These instructions and review-only agents are behavioral guidance, not permission
enforcement. See [Codex development guide](docs/development/codex.md) for execution
controls, repository navigation and verification.
