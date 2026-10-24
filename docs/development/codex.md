# Codex development workflow

This setup supports focused, evidence-driven changes, with heavier review only
where risk warrants it. It does not guarantee correctness, security or speed.
No framework implementation or release policy is replaced by these instructions.

## Discovery and invocation

Validated against **codex-cli 0.163.0**. Current official references:
[instructions](https://learn.chatgpt.com/docs/agent-configuration/agents-md),
[skills](https://learn.chatgpt.com/docs/build-skills),
[subagents and standalone TOML schema](https://learn.chatgpt.com/docs/agent-configuration/subagents),
[configuration](https://learn.chatgpt.com/docs/config-file/config-reference), and
[sandbox/approvals](https://learn.chatgpt.com/docs/agent-approvals-security).
Recheck these and `codex --version` when upgrading; do not copy legacy agent-role
tables into config alongside the standalone files.

Codex reads user instructions from its home, then project instructions from the
repository root toward the working directory. At each level a nonempty
`AGENTS.override.md` takes precedence over `AGENTS.md`; configured fallback names
apply when neither exists. More specific instructions refine earlier guidance.
Instruction loading has a size budget (normally 32 KiB). Launch at the repository
root to load the intended project context. This change adds no nested overrides.

Repository skills live in `.agents/skills/<name>/SKILL.md`. Codex scans from the
working directory up to the Git root; names/descriptions are available for
selection, and the body loads when used. Use `/skills` or type `$` in the CLI,
or explicitly request, for example:

```text
Use $scafra-security-review to inspect this router change and its denial paths.
```

Implicit selection remains enabled. Root [AGENTS.md](../../AGENTS.md) establishes
mandatory conditional triggers; skill descriptions help route matching requests.
Read only the applicable bodies. Triggers combine; a review skill does not itself
require a separate subagent for every small change.

| Task | Required workflow |
| --- | --- |
| Substantial runtime/composition redesign | Design assessment before edits; Rust and verification; compatibility/security as affected |
| Security-sensitive router modification | Security, substantive Rust review and verification; compatibility for changed public contracts; independent security reviewer |
| Procedural macro/typed graph change | Macro diagnostics, compatibility, verification and substantive Rust review; design if substantial |
| Public API/feature/dependency change | Compatibility and verification, plus reviews for the implementation's actual risks |
| Performance optimization | Performance assessment and evidence before improvement claims, Rust/verification for code; independent performance review when material |
| Documentation typo | Relevant content/link checks and `git diff --check`; no automatic Cargo or specialist reviews |

Inspect → establish evidence/contracts → choose the smallest sound solution →
implement/test → verify → independently review material/high-risk changes → fix
confirmed findings → report exact evidence and remaining risks. A short response
can contain the design assessment; ordinary work needs no planning document.

## Review agents

Standalone files in `.codex/agents/` define project agents using `name`,
`description` and `developer_instructions`. They inherit the selected parent
model/reasoning unless user defaults or explicit spawn settings select others;
this repository pins neither. Ask Codex to delegate to a name, for example:

```text
Delegate an independent review of the completed diff to scafra-independent-reviewer.
Return findings to the primary agent; do not edit files.
```

| Agent | Responsibility |
| --- | --- |
| `scafra-independent-reviewer` | Correctness, architecture, maintainability, API consequences and missing tests |
| `scafra-security-reviewer` | Trust boundaries, exploitability, policy bypasses, fail-open paths, secrets and exhaustion |
| `scafra-performance-reviewer` | Hot paths/resource lifetimes, measured regressions versus possible overhead and speculation |

All return severity, affected locations/symbols, evidence and uncertainty to the
primary agent. They must not edit, mutate remote resources, invent measurements,
or delegate further. Primary agent evaluates findings and fixes confirmed issues.
Use a specialist to cover correctness in its affected paths instead of launching
every reviewer. If subagents are unavailable, disclose the missing independence
and perform a separate local review pass. `/subagents` shows live threads in the
interactive CLI. Agent definitions are loaded for new sessions; restart after edits.

## Execution permissions

From the repository root, use:

```bash
codex --sandbox workspace-write --ask-for-approval on-request
```

The trusted-project `.codex/config.toml` sets these defaults, disables sandboxed
command networking, and caps concurrent spawned threads at two (excluding the
primary). This bounds cost/coordination without forcing delegation. Existing
user defaults and managed policy still matter; CLI overrides can change settings.
Untrusted projects skip project `.codex` configuration layers. Review the files
before granting project trust; never trust a repository merely to silence a prompt.
No user-level configuration is modified by this setup.

`workspace-write` permits automatic workspace edits/commands and normally some
temporary-directory writes. It is not a guarantee of repository-only reads or
approval before every destructive command. `on-request` lets the model request
approval when needed; it does not require approval for every allowed command.
Codex protects paths such as `.git`, `.agents` and `.codex` inside writable roots;
editing this setup can therefore require scoped approval even within the checkout.
Approve only understood, task-scoped operations. Use `--sandbox read-only` for
inspection-only sessions. Check active permissions in the client; hosted or
managed environments can use different controls. The session that installed
this setup had unrestricted execution; these defaults do not retroactively
change an existing session's permissions.

Subagents inherit the parent's permissions. Review-only instructions and
AGENTS.md are behavioral controls, not a separate sandbox. Sandbox command
network restrictions do not independently disable web search, apps or MCP tools;
inspect their permissions separately. No project command allow rules are added:
prefix rules are not a complete boundary against arbitrary scripts, wrappers or
connector mutations. User authorization is still required for commits, pushes,
merges, publication, issue edits, remote changes and destructive operations.
Do not use unrestricted access, automatic approval or broad network grants as a
workaround for failing checks. Fetch dependencies with a scoped approved action
when needed. Outside-repository writes require explicit authorization as well as
any technical permission. The existing consumer/negative/package scripts require
outside-checkout scratch paths; ask for narrow output/target path authorization
unless already supplied, or report those checks BLOCKED.

## Repository evidence and verification

The workspace has twelve coordinated crates at 0.1.0, Rust 2021, MSRV 1.88.
Foundation provides shared types; core depends on foundation and owns lifecycle
and graph contracts. Build tooling depends on core/foundation; macros use the
build generator. Config and web integrate optional companions, while the facade
composes startup. Security owns authentication/JWT/JWKS and shares policy types
through foundation. Confirm dependency edges/features in manifests when changing
them; facade defaults are empty and feature union can hide missing dependencies.

Navigate with [architecture](../architecture.md), [modules](../modules.md),
[security](../security.md), [generated code](../generated-code.md),
[resource lifecycle](../guides/resource-lifecycle.md) and
[contributing](../contributing.md). The [API policy](../api-stability.md) is
proposed, and 1.0 has not shipped. Its inventory/checklist and
[upgrading](../upgrading.md) guide compatibility assessment; historical ARCH-01
plans are not current implementation guarantees.

The [verification skill](../../.agents/skills/scafra-verification/SKILL.md) is the
command procedure. Reinspect [CI](../../.github/workflows/ci.yml) before relying on it.
Current baseline: `cargo fmt --all -- --check`,
`cargo check --workspace --all-features --locked`,
`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`,
`cargo test --workspace --all-features --locked`, plus the API policy checker and
its Python drift tests. CI also checks publish dependency order and isolated
MSRV consumers/negative configuration cases. Run affected consumers in addition
to workspace checks; all-features alone does not prove isolation.

Macro misuse uses trybuild in `crates/scafra-macros/tests/compile.rs`; graph
compile-fail/pass fixtures use a separate Cargo harness in `scafra-build/tests`.
Facade tests cover generated composition, async resources and lifecycle. Security
route-local/policy/JWT/JWKS tests and web/facade regressions exercise nested routes,
method policy and denial behavior. Use those harnesses rather than duplicating them.
Record PASSED, FAILED, BLOCKED or NOT RUN per check with reasons; source workspace
tests are distinct from path, pinned Git, staged registry and published consumers.

## Troubleshooting and extension

- Confirm working directory, Git root, `codex --version` and effective permissions.
  Check instruction overrides, size budget and user/global settings if guidance differs.
- Use `/skills` and `codex debug prompt-input "Discovery only"` to inspect loading
  locally. Prompt previews may include private context; do not publish them wholesale.
  Check YAML frontmatter, unique skill names and disabled-skill settings; restart
  if a skill update does not appear.
- For agents, check exact `name`, TOML syntax and required fields, trusted project
  status, `[agents].enabled`, available slots and user default model availability.
  Inherit an accessible model rather than pinning a new one. Use a fresh session
  and an explicit delegation request to verify actual activation, not just file presence.
- Keep shared principles in AGENTS.md, task procedures in the relevant skill and
  agent instructions limited to review responsibilities/output. Extend existing
  skills before adding overlapping ones. Validate frontmatter/TOML, references,
  discovery and representative routing after changes; report live-test limitations.
