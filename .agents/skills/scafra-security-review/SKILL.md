---
name: scafra-security-review
description: Review Scafra authentication, authorization, routing security, secrets, configuration security and untrusted-input changes for evidenced bypasses and regressions.
---

1. Read AGENTS.md and trace the changed trust boundary through source, callers,
   configuration and tests. Establish attacker input, prerequisites and intended
   access. Read docs/security.md and relevant historical regressions in
   docs/architecture/arch-01/compatibility-security.md and implementation.md
   in that directory; historical planning claims require current-source checks.
2. Inspect missing/bypassable authentication, fail-open behavior, policy precedence
   and composition: static versus parameter routes, nesting/merging, supplied/raw
   routers, managed route-local guards, inherited policies, permits and GET/HEAD
   or explicit method selection. Relevant tests include security route_local,
   policies and middleware, web optional_security/security, and facade security.
3. For JWT/JWKS, check signature/algorithm, issuer/audience/expiry, key identity,
   rotation/cache freshness, refresh failure and bounded retrieval. Invalid
   credentials must not become anonymous or another authenticator's fallback.
4. Inspect secret exposure in Display/Debug/errors/logs/tracing and config/BootUI
   output. Check unavailable/unknown configuration before merge, precedence and
   feature unification. Inspect dependency/configuration security where changed.
5. Assess input/body/response limits, timeouts, cancellation, blocking, concurrency,
   resource ownership, task/cache growth and denial-of-service failure modes.
6. Require a reproduction test for each confirmed vulnerability and regression
   tests for security-sensitive fixes, including denial paths. Never weaken an
   invariant to satisfy a test. Run relevant tests via scafra-verification.

Output prioritized severity, file:line/symbol, exploit prerequisites, execution
path, evidence, test status and correction. Label unproven concerns and uncertainty;
never call a hypothetical vulnerability confirmed. Delegate security-sensitive
implementation review as required by AGENTS.md; the primary agent evaluates it.
