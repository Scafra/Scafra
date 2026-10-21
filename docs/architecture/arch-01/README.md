# ARCH-01 Phase 0: modular framework proposal

For [issue #103](https://github.com/Scafra/Scafra/issues/103), target 1.0 Stable Core.
Status: **Implementation available for review on the current branch.**
See [implementation results](implementation.md) for verification and acceptance
status, and [modules](../../modules.md) for actual Cargo features and migration.
Phase 0 measurements remain historical evidence.

- [ADR-001](adr-001-modular-framework.md): options, proposed dependency direction,
  defaults, feature implications, config/macro strategy and maintainer decisions.
- [Source audit](dependency-audit.md) and [Cargo inventory](cargo-inventory.md):
  every crate, normal/build/dev/target edges, runtime coupling and cycle risks.
- [Baseline](baseline.md), [machine-readable results](baseline-results.json),
  and [consumer lock fixture](baseline-consumer-lock.toml): real build/dependency/
  binary/startup/memory observations and exact reproduction commands.
- [Compatibility/security](compatibility-security.md): public API migrations,
  fail-closed negative tests and lifecycle invariants.
- [Feature matrix](feature-matrix.md): isolated consumer selections and required
  absent packages. [Scripts](../../../scripts) generate consumers/measurements.
- [PR roadmap](roadmap.md): eleven packages with affected code, dependencies,
  API/security risks, tests, acceptance and Definition of Done.

Local HEAD includes #78 and its 22 regression tests pass. Required workspace
fmt/check/test/Clippy commands pass. Existing downstream test expectations have
a BootUI peer-policy mismatch (403 versus expected 200); downstream HTTP tests
pass separately. Full evidence and limits are in the baseline.

The user subsequently authorized and requested every implementation phase.
The implementation preserves #78 and the existing lifecycle contracts; the
configuration, optional web/security/actuator/JWT/JWKS/scheduler/BootUI, facade,
generator, CI and packaging work is now in the working tree. The implementation
report records final acceptance results and release-operation limitations.
No branches, commits, PRs, uploads or issue closure were created.
