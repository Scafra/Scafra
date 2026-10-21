# Proposed external consumer acceptance matrix

**Implemented feature contract.** Default follows ADR's proposed
empty default. All rows require separate consumer roots outside the workspace,
explicit feature selection and selected-host `cargo tree -e normal` evidence.
Count package identities once (name/version), not repeated tree lines. Inspect
feature trees when an unexpected edge appears. Normal trees include proc-macro
host dependencies; record build edges separately rather than calling them runtime.

| Consumer | Proposed facade features (defaults off except default row) | Normal packages that must be absent | Behavior required |
| --- | --- | --- | --- |
| Minimal | none | web, Axum, security, actuator, scheduler, BootUI, jsonwebtoken, reqwest | Core hooks, component/bean and typed properties compile/use without HTTP |
| Direct generic config | direct scafra-config, no defaults | security, actuator, scheduler, Axum, jsonwebtoken, reqwest | #75/#78 and custom application namespaces |
| Direct core | direct scafra-core | web, Axum, security, actuator, scheduler, BootUI, jsonwebtoken, reqwest | Lifecycle start/stop and graph/resource contracts |
| Web only | web | security, actuator, scheduler, BootUI, jsonwebtoken, reqwest | Generated main/controller, HTTP 200, limits/timeout/tracing, shutdown; protected declaration fails before serving |
| Secure web | security (implies web) | actuator, scheduler, BootUI, jsonwebtoken, reqwest | Basic/static bearer auth, public overrides, missing/invalid auth denial, role/scope policy |
| Local JWT | jwt | actuator, scheduler, BootUI, reqwest | Verify local tokens; reject remote config |
| Remote JWT | jwks | actuator, scheduler, BootUI | Issuer/JWKS verification, refresh, negative signatures/cache expiry |
| Health API | web + actuator | security, scheduler, BootUI, jsonwebtoken, reqwest | Health/live/ready endpoints only when selected in config, own token guard, unknown check startup failure |
| Scheduler/core | scheduler | web, Axum, security, actuator, BootUI, jsonwebtoken, reqwest | Register/start/stop scheduled tasks with application-owned runtime; no worker-runner claim |
| Default | omitted feature options | same as minimal | Default contract and diagnostics verified without feature flags |
| Standard | standard | BootUI | Existing aggregate/runtime behavior, generated main, all failure cleanup |
| BootUI only | bootui | actuator, scheduler, jsonwebtoken, reqwest | Real loopback dashboard, peer guard and safe snapshot; protected API authentication |
| All supported | standard + bootui (equivalent current proposal to --all-features) | none mandated | Link/configure BootUI once; security and effective-config snapshot policy retained |

In the table, short module names mean `scafra-MODULE`; Axum means package `axum`.
Assertions also catch unexpected JWT/JWKS client stacks introduced by Scafra;
reqwest may be a legitimate application-owned dependency in a different real
application, so use fixtures without unrelated HTTP clients for the strict gate.
Direct selected crates supply stronger isolation than facade feature subtraction.

Cargo features are unions. Add a separate consumer with two dependency paths,
one selecting web and one selecting jwks: expect the union and inspect it, never
assume `default-features=false` on one edge turns JWKS off globally. No negative
feature (`no-security`) is proposed. Feature activation enables capability; it
does not authorize endpoint exposure or authenticate requests by itself.

## Runnable isolated consumer checks

```bash
cargo fetch --locked
python3 scripts/arch103-consumers.py /var/tmp/arch103-consumers-NEW --runtime
python3 scripts/arch103-negative.py --output /var/tmp/arch103-negatives-NEW
```

The collector always enables actual requested features and enforces forbidden
normal dependencies. `--proposed` remains a compatibility flag for older commands;
it no longer means an expected failure. `--case NAME` selects one independent
consumer. Runtime mode builds and starts binaries, sends loopback requests,
checks JSON/auth/readiness, sends SIGTERM and verifies owned resource events.
Scheduler mode executes and joins task loops without Axum. BootUI-only and all
cases test the allow-listed dashboard over its own loopback listener.

The separate negative collector tests main/controller without web, protected
web-only startup rollback, feature-unified config/web subcrates rejected by the
facade capability mask before providers, and additive JWKS selection through an
aliased helper dependency. All cases retain sources/locks/trees/logs outside the
checkout. Targets may be shared, but Cargo resolution is a separate process per
consumer. No framework workspace membership or global all-features graph is used.

[Implementation results](implementation.md) record executed commands and outcomes.
