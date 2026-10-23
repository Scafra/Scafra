---
name: scafra-performance-review
description: Assess Scafra hot-path changes, optimization requests or performance claims using measurements and explicit uncertainty.
---

1. Read AGENTS.md, identify the workload and affected request/middleware, startup,
   serialization or build/macro path. Establish the actual problem before tuning.
2. Inspect allocations/clones, lock contention/synchronization, blocking async
   work, excessive spawning, dynamic dispatch, serialization and middleware
   overhead. Trace initialization, retained resources, memory growth, cleanup and
   scalability. Include compilation time/procedural macro overhead when relevant.
3. Separate demonstrated regressions from plausible overhead and speculative
   optimizations. Prefer the smallest change preserving security, correctness
   and maintainability. Do not automatically introduce benchmark infrastructure.
4. Where measurement is appropriate, specify baseline revision, candidate,
   workload/input/concurrency, tool/method, compiler/profile/features, hardware/OS,
   warmup/repetitions and variance/uncertainty. Use existing measurements/tools
   when adequate. Compare equivalent conditions; explain confounders.
5. Verify behavior and failure/resource bounds before interpreting measurements.
   Without measurements, report improvement as unverified and propose a focused
   measurement rather than asserting speed or allocation gains.

Output severity, file:line/symbol, evidence, measured results versus concerns,
trade-offs and remaining uncertainty. State exactly what ran. Independent review
for material performance-sensitive changes follows AGENTS.md.
