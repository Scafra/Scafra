---
name: scafra-macro-diagnostics
description: Validate Scafra procedural macros, typed dependency graph generation and generated application code, including diagnostics and consumer compatibility.
---

1. Read AGENTS.md, crates/scafra-macros and/or crates/scafra-build changes,
   generated-code consumers and docs/generated-code.md. Trace syn parsing and
   validation through expansion/rendering into supported application code.
2. Check source spans, actionable local errors, hygiene, qualified paths, helper
   collisions, generated readability and type correctness. Check duplicate/invalid
   declarations, missing/ambiguous dependencies, cycles, deterministic ordering,
   ownership and async resource contracts. Prefer focused diagnostics over cascades.
3. Use existing trybuild tests in crates/scafra-macros/tests/compile.rs and UI
   fixtures; inspect stderr changes rather than blindly accepting snapshots.
   Build graph tests use their own external Cargo compile-fail/pass fixtures,
   not trybuild. Extend the existing harness appropriate to the defect.
4. Cover invalid and valid consumer usage, generated facade/runtime API agreement,
   supported features and MSRV. Use facade integration tests and isolated macro
   consumers; a token snapshot alone cannot establish compatibility. Inspect
   expansion/compile-time regressions when relevant; no runtime discovery system.
5. Run targeted macro/build checks from scafra-verification plus applicable
   compatibility/security checks. Report affected inputs, expected diagnostics,
   fixture changes, exact results and unverified consumer/compile-time risks.
