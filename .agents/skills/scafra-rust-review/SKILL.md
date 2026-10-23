---
name: scafra-rust-review
description: Review substantive Scafra Rust implementation diffs for correctness, ownership, async behavior, maintainability and missing regression tests.
---

1. Read AGENTS.md, the diff, affected symbols, callers and relevant tests.
   Trace both success and failure paths across the owning crates.
2. Check idiomatic ownership/borrowing/lifetimes; explicit types; meaningful
   `Result`/`Option` handling; panic and discarded-error behavior. Check trait
   design, bounds, inference and Send/Sync requirements where they matter.
3. Check async cancellation, blocking work, task ownership, lock scope (especially
   across await), resource retention, partial initialization, rollback and shutdown.
4. Assess readability and unnecessary generics/traits/indirection, allocations,
   clones, synchronization and dynamic dispatch. Require evidence for performance
   claims, not speculative rewrites. Check testability and missing regressions.
5. Report actionable findings: severity (critical/high/medium/low), file:line,
   affected symbol, trigger, observed consequence and supporting evidence, with
   a focused correction/test suggestion. Separate uncertainty from confirmed
   defects. Avoid subjective style findings without material impact.

Return findings in priority order, checks actually performed and residual test
gaps; say explicitly when none are confirmed. Do not invent test results. The
primary agent evaluates findings and handles fixes and required verification.
