---
name: scafra-design-review
description: Review the simplest viable design before substantial Scafra architecture, runtime or application composition changes.
---

Read root AGENTS.md and the affected implementation before proposing changes.
Use docs/architecture.md and docs/modules.md as navigation, not substitutes for
source. Foundation owns shared policies, core lifecycle/typed graph contracts,
build/macros compile-time generation, companion crates capabilities, and the
facade application startup. Confirm actual dependency edges in manifests.

Produce a concise design assessment containing:

1. Actual problem and confirmed evidence (test, trace, source location or user contract).
2. Existing behavior, affected crates and consumers.
3. Invariants: ownership, deterministic graph order, initialization/rollback/
   shutdown, capability isolation, security and API/MSRV boundaries as applicable.
4. Simplest viable solution within existing boundaries.
5. Credible alternatives and concrete trade-offs, including leaving behavior
   unchanged where reasonable. Avoid architecture for hypothetical requirements.
6. Risks, failure/cancellation behavior and intended targeted validation.

A short assessment in the response is sufficient. Apply the ADR requirements in
docs/contributing.md only when their listed architectural/policy triggers apply;
reuse an applicable ADR rather than creating a planning-document hierarchy.
Resolve material uncertainty through source/tests before implementation.
