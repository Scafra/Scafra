# CFG-01 configuration and defaults audit

Source baseline: `d2bb06b` on
`111-cfg-01-audit-all-scafra-defaults-and-make-operational-settings-overridable-before-100`.
The branch spelling was explicitly confirmed by the task owner. This is a source
inventory of the implementation on that branch, not a claim that Scafra 1.0 has
shipped. [Issue 111](https://github.com/Scafra/Scafra/issues/111) is the scope;
[issue 19](https://github.com/Scafra/Scafra/issues/19) supplies the resource-protection
requirements. Existing configuration foundations from #17/#70/#75/#78, modularity
from #103, HTTP errors from #12 and the proposed compatibility policy from #16
remain the baseline.

The repository-referenced `.agents/skills` directory was absent. The task owner
authorized continuing without those skill files. Review follows AGENTS.md and
the explicit issue requirements; this does not claim those missing workflows
were loaded. See [design](cfg-01-design.md), [reference](../configuration-reference.md)
and [final review / execution ledger](cfg-01-final-review.md).

## Counting and evidence rules

There are **47 ordinary runtime property schemas** in the table below, and one
loader-owned profile selector. Per-task map fields count as schemas, not a number
of linked jobs. Struct containers, compatibility aliases, sources selecting the
same property, lower-level equivalent fields and Cargo feature names are not
counted again. Of the 47 ordinary schemas, **32 already had an application
configuration key**, and **15 gain one in this change** (eight server resource and
connection controls, two shutdown controls, four remote verifier controls and
JWT clock skew). The CLI poll setting is recorded separately. This count does
not turn existing validation defects or a changed effective default into a new
property.

`CONFIGURABLE` means supported immutable startup configuration with explicit
typed injection. It does not mean dynamic reload. All runtime rows marked **R**
support all six sources individually: **application.yaml, application.yml,
application.properties, profile files, environment, programmatic
ConfigLoader::override_value**. The [reference](../configuration-reference.md)
specifies naming, bounds, feature requirements and examples. **P** is the
profile-selector exception, whose earlier resolution and precedence are explicit.

Evidence links name the test and its scope. A resolution test is not runtime
proof. `wired` means source traced from loader through typed validation to the
consumer; `runtime test` identifies executable behavioral coverage. Test files
are not evidence of a passed command: command results and limitations are in the
final-review ledger. Where only resolution/forwarding coverage exists, that
restriction is stated rather than inferred to be a real generated application
observation.

## All twelve crates

Each manifest, owning implementation and existing regression suite was inspected.
The workspace remains at 0.1.0 with Rust 1.88 MSRV. Dependencies and feature
forwarding are compile-time contracts; `--all-features` does not establish isolated
dependency behavior.

| Crate / manifest | Implementation inspected and effective ownership | Disposition |
| --- | --- | --- |
| [scafra](../../crates/scafra/Cargo.toml) | [startup](../../crates/scafra/src/startup.rs), [private](../../crates/scafra/src/private.rs), facade exports/tests; loads final config, logs/banner, forwards RouterOptions, scheduler, extension snapshot, shutdown | Runtime forwarding and configured shutdown; dev poll override |
| [scafra-core](../../crates/scafra-core/Cargo.toml) | [lifecycle](../../crates/scafra-core/src/lifecycle.rs), graph, errors, tests; Application/resource contracts and reverse cleanup | No independent timer defaults beyond shared ShutdownPolicy; order/ownership fixed |
| [scafra-foundation](../../crates/scafra-foundation/Cargo.toml) | [logging](../../crates/scafra-foundation/src/logging.rs), backtrace, shutdown, phase, authorization, error contracts/tests | Logging already typed; ShutdownPolicy supplied by resolved shutdown config |
| [scafra-config](../../crates/scafra-config/Cargo.toml) | [models](../../crates/scafra-config/src/models.rs), [sources](../../crates/scafra-config/src/sources.rs), merge, errors, extensions and resolution/capability tests | Existing loader retained; new policies validated; alias normalization |
| [scafra-web](../../crates/scafra-web/Cargo.toml) | [routing](../../crates/scafra-web/src/routing.rs), [server](../../crates/scafra-web/src/server.rs), [limits](../../crates/scafra-web/src/limits.rs), [connection](../../crates/scafra-web/src/connection.rs), extractors/errors/tests | Streaming accounting, finite deadline, typed managed connection options retained by finalized router |
| [scafra-security](../../crates/scafra-security/Cargo.toml) | [config](../../crates/scafra-security/src/config.rs), authentication, policy, middleware, jwt, jwks and feature/security tests | JWT skew and remote operational policies configurable; cryptographic/fail-closed rules fixed |
| [scafra-actuator](../../crates/scafra-actuator/Cargo.toml) | [config](../../crates/scafra-actuator/src/config.rs), routing, registry, health, handlers, security and tests | Existing keys consumed; timeout and endpoint-name validation strengthened |
| [scafra-scheduler](../../crates/scafra-scheduler/Cargo.toml) | [lib](../../crates/scafra-scheduler/src/lib.rs), validation/config/execution/lifecycle tests | Existing per-task override already effective; fixed delay/non-overlap retained |
| [scafra-bootui](../../crates/scafra-bootui/Cargo.toml) | [lib](../../crates/scafra-bootui/src/lib.rs), binding, dashboard, snapshot, HTTP tests | Effective config handed to owned managed dashboard; shared request/connection/shutdown budgets, allowlisted snapshot and mandatory local/auth policy retained |
| [scafra-macros](../../crates/scafra-macros/Cargo.toml) | [main](../../crates/scafra-macros/src/main.rs), Config derive, controller/routes/bean/service/logger expansions and compile tests | Main uses resolved router/shutdown values; compile-time declarations remain compile-time |
| [scafra-build](../../crates/scafra-build/Cargo.toml) | [renderer](../../crates/scafra-build/src/renderer.rs), discovery, extensions, graph/parser/model and compile fixtures | Typed composition passes options, configured resource cleanup; paths/extensions belong to build APIs |
| [scafra-cli](../../crates/scafra-cli/Cargo.toml) | [cli](../../crates/scafra-cli/src/cli.rs), filesystem, generator, commands, templates, unit/generated-project tests | Starter safe values and CLI source controls; validated watcher poll flag/environment |

## Runtime inventory

Unless otherwise stated, boolean unit/range is boolean, strings have no numerical
unit, lists are sequences, and bounds are inclusive. All rows in this table are
classified **CONFIGURABLE**. Feature names refer to the facade capability;
standalone owning crates expose their own typed constructor/configuration APIs.

### Listener, request policies and shutdown

| ID / canonical key / sources | Owning source/type | Default, type, unit and validation | Effective consumer | Security/performance implications and evidence | Final source status |
| --- | --- | --- | --- | --- | --- |
| R01 `server.host` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig.host` | `127.0.0.1`, String; literal IPv4/IPv6 validated before composition | `startup::run_inner` → web bind | Non-loopback exposes interfaces. [resolution](../../crates/scafra-config/tests/resolution.rs); [server](../../crates/scafra-web/tests/server.rs) bind/HTTP tests. Host resolution itself is not DNS. | Existing; wired |
| R02 `server.port` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig.port` | `8080`, u16 TCP port; 1–65535 | `startup::run_inner` → web bind | Collision reported as bind failure. [server](../../crates/scafra-web/tests/server.rs) actual bind/drain tests; source resolution tests exercise selection. | Existing; wired |
| R03 `server.timeout` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig.request_timeout_seconds` | Now Some(30), Option<u64>, seconds; 1–86400 or explicit null; alias `request_timeout_seconds` | `startup::router_options` → `routing::with_request_timeout` | Finite occupied-request budget, cancellation and deadline-first 504. [operational resolution](../../crates/scafra-config/tests/operational.rs) defaults/alias/precedence; [facade runtime](../../crates/scafra/tests/operational_config.rs) configured 504 and opt-out; [web resources](../../crates/scafra-web/tests/resource_limits.rs) route/cancellation/ties. | Existing key; effective default corrected; wired/runtime tests |
| R04 `server.max_request_body_bytes` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/limits.rs](../../crates/scafra-web/src/limits.rs) | `1048576`, usize bytes; 1–1073741824 | RouterOptions → DefaultBodyLimit + streaming `Limited<Body>` guard | Bounded consumed bytes without full buffering; unknown unread bytes cannot produce universal 413. [web resources](../../crates/scafra-web/tests/resource_limits.rs) exact/over/raw/JSON/chunked/discarded-error cases; [facade runtime](../../crates/scafra/tests/operational_config.rs). | New key; fixed extractor-only policy replaced; runtime tests |
| R05 `server.header_read_timeout_seconds` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/connection.rs](../../crates/scafra-web/src/connection.rs) `ConnectionOptions` | `10`, u64 seconds; 1–3600 | FinalizedRouter retained connection options → Hyper HTTP/1 `header_read_timeout` | Bounds incomplete HTTP/1 headers before routing, not HTTP/2 absolute headers. [web resources](../../crates/scafra-web/tests/resource_limits.rs) slow-header TCP test; [facade forwarding](../../crates/scafra/tests/operational_config.rs). | New key; wired/runtime test |
| R06 `server.idle_timeout_seconds` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/connection.rs](../../crates/scafra-web/src/connection.rs) `IdleIo` | `60`, u64 seconds; 1–86400 | Managed connection transport progress timer | Bounds silence before/during connection I/O; activity refreshes it. [web resources](../../crates/scafra-web/tests/resource_limits.rs) idle TCP test; [facade forwarding](../../crates/scafra/tests/operational_config.rs). | New key; wired/runtime test |
| R07 `server.max_connections` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/connection.rs](../../crates/scafra-web/src/connection.rs) | `1024`, usize concurrent connections; 1–1000000 | Managed accept-loop gate on JoinSet length per listener | Bounds owned connection tasks; acceptance pauses at capacity, not per-IP limiting. [web resources](../../crates/scafra-web/tests/resource_limits.rs) `admission_ceiling_defers_next_connection_until_slot_is_released`; config resolution/forwarding separately. | New key; wired/runtime test |
| R08 `server.max_header_bytes` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/connection.rs](../../crates/scafra-web/src/connection.rs) | `32768`, usize bytes; 8192–1048576 | HTTP/1 `max_buf_size`; HTTP/2 `max_header_list_size` | Parser buffer and decoded list have different accounting. [web resources](../../crates/scafra-web/tests/resource_limits.rs) `header_buffer_ceiling_and_body_source_failure_remain_bounded`; config resolution/forwarding separately. | New key; wired/runtime test |
| R09 `server.http1_keep_alive` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/connection.rs](../../crates/scafra-web/src/connection.rs) | `true`, bool | Hyper HTTP/1 `keep_alive` | Reuse reduces connection setup but retains transport resources. [web resources](../../crates/scafra-web/tests/resource_limits.rs) real connection-close test after finalization. | New key; wired/runtime test |
| R10 `server.http1_max_headers` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/connection.rs](../../crates/scafra-web/src/connection.rs) | `100`, usize header fields; 1–1000 | Hyper HTTP/1 `max_headers` | Bounds parser header-field work separately from bytes; default remains Hyper's historical100. [config operational](../../crates/scafra-config/tests/operational.rs) source/bounds and facade forwarding; runtime evidence recorded in ledger. | New key; wired |
| R11 `server.http2_max_concurrent_streams` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ServerConfig`; [web/connection.rs](../../crates/scafra-web/src/connection.rs) | `200`, u32 concurrent streams per connection; 1–10000 | Hyper HTTP/2 `max_concurrent_streams` | Adds a per-connection request concurrency ceiling for HTTP/2; connection count alone is insufficient. [web resources](../../crates/scafra-web/tests/resource_limits.rs) `configured_header_count_and_http2_stream_concurrency_are_effective` observes only one handler until the first stream releases; config source/bounds tests. | New key; wired/runtime test |
| R12 `shutdown.grace_period_seconds` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ShutdownConfig`; [foundation/shutdown.rs](../../crates/scafra-foundation/src/shutdown.rs) | `30`, u64 seconds; 1–86400 | `ShutdownConfig::policy` → runner, web drain, lifecycle, generated resource cleanup | HTTP drain enforces the deadline; resource callbacks receive advisory policy and implement their own deadlines. No preemption of blocking callbacks. [facade runtime](../../crates/scafra/tests/operational_config.rs) pending HTTP request with configured grace; generated lifecycle tests cover cleanup paths. | New key; wired/runtime test |
| R13 `shutdown.force_after_grace` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `ShutdownConfig`; [foundation/shutdown.rs](../../crates/scafra-foundation/src/shutdown.rs) | `true`, bool | Same policy consumers select bounded/unbounded waiting | False is explicit unbounded waiting; true cannot undo external effects. [operational resolution](../../crates/scafra-config/tests/operational.rs), [web shutdown tests](../../crates/scafra-web/src/tests.rs), [core tests](../../crates/scafra-core/src/tests.rs) explicit policy; ledger distinguishes each path executed. | New key; wired |

### Foundation and startup

| ID / canonical key / sources | Owning source/type | Default/type/unit and validation | Effective consumer | Security/performance implications and evidence | Final source status |
| --- | --- | --- | --- | --- | --- |
| R14 `logging.level` · R | [foundation/logging.rs](../../crates/scafra-foundation/src/logging.rs) `LoggingConfig.level` | Info, LogLevel/string; off/error/warn/info/debug/trace with documented aliases | Runner → foundation `init_runtime_logging` → EnvFilter | `RUST_LOG` can supersede level; diagnostics volume. Generated runtime script debug mode proves configured debug output; foundation unit `invalid_rust_log_falls_back_without_echoing_input`, [config matrix](../../crates/scafra-config/tests/ember014_matrix.rs) typed level resolution; custom subscriber ownership retained. | Existing; wired |
| R15 `logging.backtrace` · R | [foundation/logging.rs](../../crates/scafra-foundation/src/logging.rs) `LoggingConfig.backtrace` | Off, BacktraceMode/string; off/errors and aliases | Foundation logging initialization → `backtrace::set_backtrace_enabled` OnceLock | Server-side capture cost; first initialization only. [runtime_policy](../../crates/scafra-foundation/tests/runtime_policy.rs) first policy; [web redaction](../../crates/scafra-web/tests/backtrace_redaction.rs). | Existing; wired/runtime tests |
| R16 `startup.banner` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `StartupConfig.banner` | `true`, bool | Runner guards `print_startup_banner` | Controls stdout startup presentation; logging remains independent. [config matrix](../../crates/scafra-config/tests/ember014_matrix.rs) and generated applications use startup settings; dedicated banner-output matrix is not implied. | Existing; wired; output-specific coverage limited |
| R17 `startup.show_config` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `StartupConfig.show_config` | `true`, bool | `startup::print_startup_banner` | Prints host/port/profile/log level/endpoints only, not credentials. Same source/test limitation as R16. | Existing; wired; output-specific coverage limited |

### Security

| ID / canonical key / sources | Owning source/type | Default/type/unit and validation | Effective consumer | Security/performance implications and evidence | Final source status |
| --- | --- | --- | --- | --- | --- |
| R18 `security.enabled` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `SecurityConfig` | `false`, bool; true requires authenticator; security feature | Middleware `SecurityContext`/`enforce` | Explicit protected routes cannot become anonymous when disabled. [feature tests](../../crates/scafra-security/tests/features.rs), [route-local tests](../../crates/scafra-security/tests/route_local.rs). | Existing; wired/runtime tests |
| R19 `security.hide_unauthorized` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `SecurityConfig` | `false`, bool; security feature | Middleware `denied`/`misconfigured` | Conceals denial as 404, never grants access. [middleware](../../crates/scafra-security/tests/middleware.rs) hidden denial tests. | Existing; wired/runtime tests |
| R20 `security.bearer_token` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `SecurityConfig` | None, Option<String>; nonblank when enabled; security feature | `authentication::authenticate` | Credential exact-match; Debug-redacted, serialization still supports it. [middleware](../../crates/scafra-security/tests/middleware.rs) valid/invalid/malformed tokens; [config](../../crates/scafra-security/tests/config.rs) redaction. | Existing; wired/runtime tests |
| R21 `security.basic.username` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `BasicAuthConfig` | None, Option<String>; complete pair required; configured empty rejected when enabled | `authentication::authenticate` | Basic requires transport protection; username Debug-redacted. [middleware](../../crates/scafra-security/tests/middleware.rs) matching decoded credentials. | Existing; security feature; wired/runtime test |
| R22 `security.basic.password` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `BasicAuthConfig` | None, Option<String>; complete pair required; configured empty rejected when enabled | `authentication::authenticate` | Omitted in serialization/Debug; roles not synthesized from credentials. Same Basic middleware test and config serialization test. | Existing; security feature; wired/runtime test |
| R23 `security.permit_all` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `SecurityConfig` | `[]`, Vec<String>; exact paths | `is_permitted` → middleware `enforce` using OriginalUri | Applies to inherited global policy; cannot remove explicit requirements. [route_local](../../crates/scafra-security/tests/route_local.rs) original external URI; [features](../../crates/scafra-security/tests/features.rs) inherited role/scope bypass regression. | Existing; security feature; wired/runtime tests |
| R24 `security.jwt.enabled` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `false`, bool; enabled requires secret or provider; jwt feature | `authentication::authenticate` selects JWT | Missing capability never falls back to another scheme. [features](../../crates/scafra-security/tests/features.rs) and [JWT runtime](../../crates/scafra-security/tests/jwt.rs). | Existing; wired/runtime tests |
| R25 `security.jwt.secret` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | None, Option<String>; nonblank enabled JWT, incompatible with direct remote key endpoint | `jwt::validate_jwt` local HS256 | Secret omitted serialization/Debug, mandatory signature check. [jwt](../../crates/scafra-security/tests/jwt.rs) signature/algorithm/claims failures; [config](../../crates/scafra-security/tests/config.rs) redaction. | Existing; jwt feature; wired/runtime tests |
| R26 `security.jwt.issuer_uri` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | None, Option<String>; valid HTTP(S) URL without credentials/fragment/query for enabled JWT | `jwt::validate_jwt` issuer; `jwks::fetch_jwks` discovery without local secret | Local issuer restriction works without jwks; remote discovery needs jwks; HTTPS advised. [jwt](../../crates/scafra-security/tests/jwt.rs), [jwks](../../crates/scafra-security/tests/jwks.rs) actual OIDC discovery. | Existing; wired/runtime tests |
| R27 `security.jwt.jwk_set_uri` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | None, Option<String>; HTTP(S), no credentials/fragment; jwks feature | `jwks::fetch_jwks` direct endpoint | Restricts destination; URLs Debug-redacted; redirects disabled. [jwks](../../crates/scafra-security/tests/jwks.rs) retrieval/provider failure/malformed/oversize. | Existing; wired/runtime tests |
| R28 `security.jwt.audiences` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `[]`, Vec<String>; no blank entries in enabled JWT; jwt feature | `Validation::set_audience` in jwt | Claims restriction; empty list does not disable cryptography. [jwt](../../crates/scafra-security/tests/jwt.rs) issuer/audience/expiration. | Existing; wired/runtime tests |
| R29 `security.jwt.required_scopes` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `[]`, Vec<String>; enabled JWT rejects blank entries; all required on authenticated JWT; jwt feature | Middleware `enforce` combined scope iterator | Combines route requirements; blank entries now fail startup without credential diagnostics. [security config](../../crates/scafra-security/tests/config.rs) blank scope regression; [jwt](../../crates/scafra-security/tests/jwt.rs) missing configured scope403; [route_local](../../crates/scafra-security/tests/route_local.rs) nested requirements. | Existing; wired/runtime tests |
| R30 `security.jwt.leeway_seconds` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `60`, u64 seconds; 0–300; jwt feature | `jwt::validate_jwt` → `Validation.leeway` | Explicitly bounded clock skew; no signature/expiry bypass. [JWT runtime](../../crates/scafra-security/tests/jwt.rs) `explicit_clock_skew_changes_expiry_and_not_before_acceptance`. | New key replaces implicit library default; runtime test |
| R31 `security.jwt.cache_ttl_seconds` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `300`, u64 seconds; 1–86400; jwks feature | SecurityContext → `JwksCache::new` → `cached_key` freshness | Key freshness/provider-load tradeoff; never accepts expired key after failed refresh. [jwks](../../crates/scafra-security/tests/jwks.rs) `configured_cache_lifetime_expires_keys_and_fails_closed_on_refresh_failure`. | New key; wired/runtime test |
| R32 `security.jwt.refresh_cooldown_seconds` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `30`, u64 seconds; 1–3600 and ≤ TTL; jwks feature | `JwksCache::new` → `get_key` refresh gates | Bounds failed/unknown-key request-triggered refresh load. [jwks](../../crates/scafra-security/tests/jwks.rs) `configured_cooldown_limits_unknown_key_refreshes_and_then_allows_rotation`. | New key; wired/runtime test |
| R33 `security.jwt.connect_timeout_ms` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `2000`, u64 milliseconds; 1–300000 and ≤ request deadline; jwks feature | `JwksCache::new` → reqwest builder connection timeout | Bounds stalled transport/TLS setup. [jwks](../../crates/scafra-security/tests/jwks.rs) `configured_connect_deadline_bounds_a_stalled_tls_handshake`, actual local TCP peer. | New key; wired/runtime test |
| R34 `security.jwt.request_timeout_ms` · R | [security/config.rs](../../crates/scafra-security/src/config.rs) `JwtConfig` | `5000`, u64 milliseconds; 1–300000 and ≥ connect deadline; jwks feature | reqwest deadline plus `fetch_jwks` outer timeout over discovery+keys | Bounds total remote authentication operation; timeout denied generically. [jwks](../../crates/scafra-security/tests/jwks.rs) `configured_provider_deadline_cancels_slow_retrieval_without_disclosing_provider_data`. | New key; wired/runtime test |

### Actuator, scheduler and BootUI

| ID / canonical key / sources | Owning source/type | Default/type/unit and validation | Effective consumer | Security/performance implications and evidence | Final source status |
| --- | --- | --- | --- | --- | --- |
| R35 `actuator.endpoints` · R | [actuator/config.rs](../../crates/scafra-actuator/src/config.rs) `ActuatorConfig` | Empty names list, EndpointSelection list/string; known endpoint names or `*` | Actuator routing builds aliases/reservations | Selects operational exposure; unknown names rejected. [config](../../crates/scafra-actuator/tests/config.rs) unknown values/redaction; [routing](../../crates/scafra-actuator/tests/routing.rs) installed groups. | Existing; validation corrected; runtime tests |
| R36 `actuator.health.checks` · R | [actuator/config.rs](../../crates/scafra-actuator/src/config.rs) `HealthConfig` | `[]`, Vec<String>; empty/`*` all; others must identify registry checks | Registry selection → health evaluator | Unknown readiness names cannot produce UP; sequential evaluation. [registry](../../crates/scafra-actuator/tests/registry.rs), [health](../../crates/scafra-actuator/tests/health.rs); [facade startup](../../crates/scafra/tests/actuator_startup.rs) profile/environment selection. | Existing; wired/runtime tests |
| R37 `actuator.health.check_timeout_ms` · R | [actuator/config.rs](../../crates/scafra-actuator/src/config.rs) `HealthConfig` | `1000`, u64 milliseconds per check; 1–300000 | `health::evaluate` Tokio timeout | Drops cooperative future; does not preempt blocking poll; aggregate may take sum. [registry](../../crates/scafra-actuator/tests/registry.rs) `custom_timeout_changes_the_health_result_for_the_same_check`, invalid direct entrypoints; [config operational](../../crates/scafra-config/tests/operational.rs) environment validation. | Existing; bounds corrected; wired/runtime tests |
| R38 `actuator.security.enabled` · R | [actuator/config.rs](../../crates/scafra-actuator/src/config.rs) `ActuatorSecurity` | `false`, bool; enabled needs nonblank token | `security::protected` before health/ready/live evaluation | Separate operational access boundary. [security](../../crates/scafra-actuator/tests/security.rs) missing/invalid/valid/duplicate credentials. | Existing; actuator feature; wired/runtime tests |
| R39 `actuator.security.bearer_token` · R | [actuator/config.rs](../../crates/scafra-actuator/src/config.rs) `ActuatorSecurity` | None, Option<String>; nonblank when enabled | Same actuator authenticator | Omitted serialization, not emitted in HTTP/errors. Same security tests; [config token validation](../../crates/scafra-config/tests/actuator_security.rs) environment path. | Existing; wired/runtime tests |
| R40 `scheduler.enabled` · R | [scheduler/lib.rs](../../crates/scafra-scheduler/src/lib.rs) `SchedulerConfig` | `false`, bool | Runner/direct `Scheduler::try_start` filters registrations | No loops when disabled; no-task enabled is valid. [config](../../crates/scafra-scheduler/tests/config.rs), [execution](../../crates/scafra-scheduler/tests/execution.rs). | Existing; scheduler feature; wired/runtime tests |
| R41 `scheduler.tasks.<name>.enabled` · R | [scheduler/lib.rs](../../crates/scafra-scheduler/src/lib.rs) `ScheduledTaskConfig` | `true`, bool; name must be a linked registration | `try_start` per-task selection | Prevents starting disabled task; map cannot dynamically register code. [execution](../../crates/scafra-scheduler/tests/execution.rs) deterministic enabled/disabled/cancellation. | Existing; wired/runtime test |
| R42 `scheduler.tasks.<name>.interval_ms` · R | [scheduler/lib.rs](../../crates/scafra-scheduler/src/lib.rs) `ScheduledTaskConfig` | None, Option<u64>; registration fallback; 1–31536000000 ms when supplied | `try_start` → `run_task` first/next-run calculation | Fixed-delay/no-overlap prevents callback backlog. [execution](../../crates/scafra-scheduler/tests/execution.rs) interval effects; [config](../../crates/scafra-scheduler/tests/config.rs) bounds; [environment matrix](../../crates/scafra-config/tests/ember014_matrix.rs) dynamic task keys. | Existing; wired/runtime test |
| R43 `bootui.enabled` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `BootUiConfig` | `false`, bool | Runner → `start_managed_with_config` → `prepare`/managed router | No dashboard listener unless enabled; bootui feature. [bootui HTTP](../../crates/scafra-bootui/tests/bootui_http.rs) `owned_dashboard_uses_effective_resource_limits_and_is_joined_on_shutdown`. | Existing; wired/runtime test |
| R44 `bootui.host` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `BootUiConfig` | `127.0.0.1`, String literal IP when enabled; loopback when local_only; validated before application bind | `binding::resolve` → separate TcpListener | No hostname resolution; remote binding requires auth policy. [bootui HTTP](../../crates/scafra-bootui/tests/bootui_http.rs) invalid/non-loopback host and peer cases. | Existing; wired/runtime tests |
| R45 `bootui.port` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `BootUiConfig` | `8091`, u16 TCP port; 1–65535 | `binding::resolve` → separate TcpListener | Address independent of application server; bind errors logged. [bootui HTTP](../../crates/scafra-bootui/tests/bootui_http.rs) owned dashboard binds configured port, applies8-byte413/header deadline and releases listener on joined shutdown. | Existing; wired/runtime test |
| R46 `bootui.path` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `BootUiConfig` | `/bootui`, String; non-root static absolute path validated before bind; rejects control bytes/patterns/query/fragment/backslash | `try_layer_with_config_and_profile`/`prepare` mount | Route shape must remain valid; not a route-auth bypass. [bootui HTTP](../../crates/scafra-bootui/tests/bootui_http.rs) actual dashboard route tests. | Existing; wired/runtime tests |
| R47 `bootui.local_only` · R | [config/models.rs](../../crates/scafra-config/src/models.rs) `BootUiConfig` | `true`, bool; false requires enabled valid application authentication | `binding::resolve`/`protect_router` | Enabled config validates policy before application bind; true checks bind+peer; false requires valid authentication and clears dashboard permit exceptions. [bootui HTTP](../../crates/scafra-bootui/tests/bootui_http.rs) authenticated remote-policy route and local peer failures. | Existing; wired/runtime tests |
| R48 `scafra.profiles.active` · P | [config/sources.rs](../../crates/scafra-config/src/sources.rs) loader `profile_from_base` | Absent; effective profile name `default`; string safe-name validation | Selects application-profile source files before ordinary environment/property merging | `.profile()` > SCAFRA_PROFILE > base field; no late override changes earlier file selection. [config resolution](../../crates/scafra-config/tests/resolution.rs), [facade config](../../crates/scafra/tests/config.rs). | Existing loader control; explicitly different precedence |

## CLI/build, invariants and private candidates

These rows are explicit exceptions to the ordinary application-property source
contract. A supported CLI override is not falsely described as an application
YAML override. The public config reference explains the associated usage.

| ID / candidate / classification | Source and verified default | Mechanism or concrete reason for exception | Evidence/status |
| --- | --- | --- | --- |
| X01 Cargo capability selections · COMPILE_TIME | [facade manifest](../../crates/scafra/Cargo.toml), all twelve manifests; facade/default feature sets empty | Dependency inclusion and generated API availability determined by Cargo; runtime keys cannot enable uncompiled security/JWT/HTTP. | Existing feature/capability tests; independent consumers required |
| X02 Controller paths, route policies and macro syntax · COMPILE_TIME | [macros/controller.rs](../../crates/scafra-macros/src/controller.rs), routes/service/bean | Macros generate Rust types and routes; changing declarations requires compiling. Runtime policy values still use typed security config. | trybuild/graph fixtures and nested authorization regressions |
| X03 Config derive prefix/required annotations · COMPILE_TIME | [macros/config.rs](../../crates/scafra-macros/src/config.rs); no default prefix, required only by annotation | Field validation/type checking emitted at compile time; values themselves load at runtime. | Config derive positive/negative compile tests |
| X04 Scheduled registration baseline · COMPILE_TIME | [scheduler/lib.rs](../../crates/scafra-scheduler/src/lib.rs) `register_scheduled_task!`; interval supplied by registration | Linked executable code/name cannot be created from YAML. Runtime interval/enable overrides R41/R42 remain supported. | Scheduler validation/execution tests |
| X05 Source discovery and build context · CLI_OR_BUILD | [build/lib.rs](../../crates/scafra-build/src/lib.rs), extensions; conventional main source root `src/main` | Explicit source-root/build-context APIs and Cargo manifest/OUT_DIR controls run during compilation; not runtime settings. | Discovery and compile fixture tests |
| X06 Generated module filename/staging maps · INTERNAL | [build/lib.rs](../../crates/scafra-build/src/lib.rs) `scafra_modules.rs`; extensions owned BTreeMap | Artifact name is a generated-code protocol, map capacity grows with trusted build output; no application availability policy. | Build output/path/extension tests |
| X07 Development polling · CLI_OR_BUILD | [cli/filesystem.rs](../../crates/scafra-cli/src/filesystem.rs), [facade/private.rs](../../crates/scafra/src/private.rs); formerly hardcoded 500 ms | Now flag > SCAFRA_DEV_POLL_INTERVAL_MS > 500 ms, validated100–60000ms. Separate CLI/debug process; app YAML cannot govern its parent supervisor. | CLI unit `development_poll_interval_has_validated_precedence_and_redacted_errors`, direct parser test, invalid CLI process test |
| X08 Starter kind/features · CLI_OR_BUILD | [cli/cli.rs](../../crates/scafra-cli/src/cli.rs), templates; web kind/web capability | CLI flags choose source/Cargo generation; requires recompilation by definition. Templates list actual runtime defaults separately. | CLI template/generated application tests |
| X09 Dev config source flags · CLI_OR_BUILD | [cli/cli.rs](../../crates/scafra-cli/src/cli.rs) config-dir/config-file absent | Mutually exclusive CLI selectors forwarded to runtime loader; controls source choice rather than a key inside the chosen source. | CLI tests and config resolution matrix |
| X10 Embedded development reload · CLI_OR_BUILD | [macros/main.rs](../../crates/scafra-macros/src/main.rs) debug enabled, SCAFRA_DEV_RELOAD=false opt-out | Parent supervises binary before application startup; runtime YAML is not the parent process contract. Release builds do not supervise. | Generated app/dev tests; explicit source branch |
| X11 DEV_CHILD marker · INTERNAL | [facade/private.rs](../../crates/scafra/src/private.rs), CLI child env | Prevents recursive dev supervisors; not supported application behavior knob. | Child spawn source and CLI generated-project tests |
| X12 Executor infrastructure · INTERNAL | [macros/main.rs](../../crates/scafra-macros/src/main.rs) Tokio multi-thread builder defaults | Executor exists before Scafra startup. CPU-based worker count (Tokio also recognizes TOKIO_WORKER_THREADS), blocking-pool capacity/idle policy and platform stack size belong to embedding runtime/custom main, not supported Scafra application policy. Use a custom typed Tokio builder to own them. | Explicit exception for independent-review scrutiny; no claim YAML can affect them |
| X13 Logging subscriber format/ANSI · INTERNAL | [foundation/logging.rs](../../crates/scafra-foundation/src/logging.rs), [facade/startup.rs](../../crates/scafra/src/startup.rs); existing formatter and terminal/NO_COLOR behavior | Presentation contract, custom subscriber supported through tracing initialization; no mutable global format registry. | Foundation logging tests; `issue111-runtime.py` banner/compact/debug modes verify actual generated output |
| X14 RUST_LOG tracing directive · CONFIGURABLE (environment/embedding) | [foundation/logging.rs](../../crates/scafra-foundation/src/logging.rs); absent uses logging.level | Existing advanced tracing environment contract supersedes simple level; explicit lower-level textual directive API also exists. Not counted as another canonical application property. | Invalid directive fallback/redaction unit tests |
| X15 Backtrace/subscriber first initialization · FIXED_INVARIANT | [foundation/backtrace.rs](../../crates/scafra-foundation/src/backtrace.rs), logging try_init | Process-global diagnostic initialization is one-time; reinitializing subscribers unsafely or mutating backtrace state per request is not supported. | runtime_policy/concurrent_initialization tests |
| X16 Lifecycle order/resource ownership · FIXED_INVARIANT | [core/lifecycle.rs](../../crates/scafra-core/src/lifecycle.rs), generated resource set | Deterministic startup/reverse cleanup and ownership ensure resources outlive consumers. Settings cannot disable required cleanup/order. Policy is configurable separately. | Core/facade lifecycle tests |
| X17 Scheduler non-overlap and fixed delay · FIXED_INVARIANT | [scheduler/lib.rs](../../crates/scafra-scheduler/src/lib.rs); first after interval, next after completion | Existing single-callback-per-registration semantics avoid unbounded backlog; no speculative modes/distributed scheduler added. Registered task count is compile/link time. | execution tests verify coalescing/non-overlap/panic/cancellation |
| X18 Scheduler name byte cap · FIXED_INVARIANT | [scheduler/lib.rs](../../crates/scafra-scheduler/src/lib.rs); 64 ASCII bytes | Bounds diagnostic identifiers and defines valid linked registry keys; not a runtime capacity budget. | registration_validation/config tests |
| X19 JWKS provider response cap · FIXED_INVARIANT | [security/jwks.rs](../../crates/scafra-security/src/jwks.rs); 1048576 bytes per response | Protects trusted-provider failure/malicious response memory and parsing work; byte accounting applies while reading response chunks; no unlimited option. | jwks oversized-response runtime test |
| X20 JWKS redirect policy · FIXED_INVARIANT | [security/jwks.rs](../../crates/scafra-security/src/jwks.rs); redirects disabled | Redirects can change authentication key destination; explicit configured URI/discovery selection required. | Source-enforced destination policy; not a user-tunable retry mode |
| X21 Remote fetch serialization/private pooling · INTERNAL | [security/jwks.rs](../../crates/scafra-security/src/jwks.rs); one refresh gate per context, client defaults | Serialized provider fetches limit active requests. Pool capacity/reuse is private to fixed provider destinations, not an independent framework concurrency policy; cache/cooldown/deadlines are exposed. | Concurrent unknown-key refresh test; no performance improvement claim |
| X22 Cryptography and fail-closed authorization · FIXED_INVARIANT | [security/jwt.rs](../../crates/scafra-security/src/jwt.rs), middleware; HS256 local, supported RSA remote, mandatory exp/signature/nbf | A harmless-looking key cannot disable signature/required-claim validation, make missing roles/scopes pass, remove protected-route declarations or enable uncompiled verifiers. | jwt/jwks/features/policies/route_local negative suites |
| X23 Credential redaction and BootUI snapshot allowlist · FIXED_INVARIANT | [bootui/snapshot.rs](../../crates/scafra-bootui/src/snapshot.rs), config/error implementations | Snapshot includes server host/port/profile only. Future or profile/environment credentials cannot enter it by arbitrary serialization. | bootui HTTP snapshot/profile/future-credential tests; security/config diagnostics tests |
| X24 Local-only/remote BootUI authentication checks · FIXED_INVARIANT | [bootui/binding.rs](../../crates/scafra-bootui/src/binding.rs); loopback peer or enabled authentication | local_only itself is an explicit policy setting, but each branch's enforcement is non-negotiable and remote mode clears permit-all exceptions. | BootUI local/remote route negative tests |
| X25 HTTP framing/status/header semantics · FIXED_INVARIANT | Hyper plus [web/errors.rs](../../crates/scafra-web/src/errors.rs), foundation/http_error | Parser owns declared HTTP message boundaries; HEAD, WWW-Authenticate, Allow and stable error codes must survive policy middleware. | HTTP error/security/HEAD regression suites |
| X26 HTTP/2 absolute header deadline/absolute lifetime/per-IP admission/TLS · INTERNAL (unsupported policy APIs) | [web/connection.rs](../../crates/scafra-web/src/connection.rs), current Hyper adapter | No such Scafra setting/guarantee implemented. HTTP/1 header deadline, transport inactivity and header bounds are explicit. Additional ingress policy belongs to embedding server/reverse proxy. Do not advertise nonexistent knobs. | Explicit limitation; protocol coverage recorded in final report |
| X27 BootUI listener ownership · INTERNAL (shared runtime controls) | [bootui/lib.rs](../../crates/scafra-bootui/src/lib.rs) `BootUiServer`/`start_managed_with_config` | Dashboard has a separate bind but reuses scafra-web managed HTTP stack and all R03–R13 budgets. Runner requests shutdown and joins before releasing resources; Drop aborts cooperative serving. No duplicate dashboard resource-policy namespace is necessary. User-owned route embedding owns its server policy. | `owned_dashboard_uses_effective_resource_limits_and_is_joined_on_shutdown` actual8-byte413/header timeout/configured bind/joined shutdown |
| X28 Loader source/prefix selectors · CONFIGURABLE (loader control, not value key) | [config/sources.rs](../../crates/scafra-config/src/sources.rs); SCAFRA prefix, cwd/resources convention | ConfigLoader::root/config_dir/file/env_prefix and explicit env selectors choose inputs. Documented separate from resolved ScafraConfig. | resolution/ember014_matrix tests |
| X29 Listener accept retry backoff · INTERNAL | [web/connection.rs](../../crates/scafra-web/src/connection.rs); one second on accept errors | Recovery from aborted peers/descriptor exhaustion must neither terminate the application nor spin the executor. Backoff is interruptible by shutdown and does not delay existing connections. It is an error-recovery implementation constant rather than a supported application retry policy. | Independent review correction; source-enforced retry/error redaction |
| X30 Generator staging collision retries · INTERNAL | [cli/generator.rs](../../crates/scafra-cli/src/generator.rs); bounded unique staging-name attempts | Private filesystem race recovery for trusted CLI output, not application resource concurrency. Atomic no-overwrite promotion remains an invariant; choose a different output path on exhausted collisions. | Existing generator collision/no-overwrite tests |


## Confirmed gaps and disposition

| Finding against baseline | Resolution and scope | Evidence / remaining boundary |
| --- | --- | --- |
| Fixed 1MiB extractor-only body policy | R04 exposes validated hard ceiling and streaming byte limiter; keep extractor support | Raw/JSON/exact/over/chunked/error-swallowing tests. Unread data and already committed responses are explicitly scoped. |
| No default handler deadline | R03 defaults to30s; explicit null preserves intentional unlimited mode | Config omission/null/precedence and real504/custom/disabled tests. Not arbitrary code preemption. |
| Convenience server did not expose connection builder controls | R05–R11 use existing Hyper stack's supported controls plus bounded admission/inactivity | Finalized router retains options; raw Axum embedding transfers server responsibility. No invented H2 header deadline. |
| Runner/cleanup uses fixed shutdown policy | R12/R13 wire resolved policy; generated composition gets a policy-taking path | Cleanup/HTTP deadline tests and source wiring. Synchronous hooks/callbacks still cannot be force-stopped. |
| JWKS timing/cache hardcoded | R31–R34 preserve defaults while exposing validated policy | Four non-default network/cache behavioral tests including stalled TLS handshake. |
| JWT inherited library clock skew | R30 makes60s default explicit/bounded and overridable | Expiry/nbf accept vs deny with60s vs0s; mandatory checks retained. |
| Actuator accepts zero/huge health deadline | R37 validates1–300000ms in config and standalone registry/router entrypoints | Invalid empty-registry and legacy/direct entrypoint tests; normal short/long behavior. |
| Unsupported actuator endpoint values silently disappear | R35 validates endpoint names | Safe unknown-selection diagnostic; existing group routing behavior retained. |
| Development watch fixed500ms | X07 exposes CLI/environment setting with bounds | CLI/unit/direct invalid process tests. Not application YAML. |
| BootUI diagnostic listener lacked shared resource/drain policy | Reuses the managed web stack with the effective server and shutdown budgets, retained by a typed owner in the standard runner | Owned dashboard real TCP limit/header/drain test; lower-level route-only embedding still transfers server ownership. |
| String-list leaf values failed properties/environment/programmatic decoding | Accept lists at schema-list leaves while rejecting mappings and positional struct sequences | `supported_string_lists_work_from_all_sources_and_reject_invalid_elements` covers permit_all, audiences, scopes, health checks and endpoint selection across all six sources with redacted failures. |
| Blank required JWT scopes accepted until runtime denial | Reject blank required scopes in enabled JWT validation | `enabled_jwt_rejects_blank_required_scopes_without_disclosing_credentials`; no change to exact permit_all matching. |
| Timeout alias can collide with canonical serialized default | Normalize alias before precedence; reject same-source conflicts | operational config alias file/profile/environment/programmatic and duplicate tests. |

## Limitations that must remain visible in release evidence

The runtime table deliberately differentiates source wiring, direct middleware
tests, real TCP tests and generated-entrypoint tests. Dedicated banner-output and every individual existing property across every source
are not established merely by existing struct tests. The final report must
state whether the required broader source/runtime/generated/consumer matrix was
actually executed and identify any remaining release-blocking acceptance gaps.

Router body accounting has one small shared exceeded-state allocation per
installed ceiling; the transport adapter owns one inactivity timer and task per
accepted connection. No full request buffering, global locks or mutable config
registry are introduced. This is source analysis, not measured throughput or
latency improvement. Background effects, blocking code and user-owned servers
are outside cooperative timeout guarantees. Feature isolation and Rust1.88 must
be validated independently of a successful workspace feature-union build.


The module source matrix in
[operational.rs](../../crates/scafra-config/tests/operational.rs),
`module_settings_share_the_file_profile_environment_and_programmatic_contract`,
exercises logging/backtrace, startup flags, security concealment, JWT skew and all
four JWKS operational fields, actuator deadline, scheduler enablement and BootUI
port/local-only through YAML, YML, properties, profiles, real environment values
and programmatic overrides. This is configuration-resolution evidence; the
module-specific runtime tests above supply behavioral evidence where present.

The managed auto builder enables HTTP/2 in the normal web dependency closure.
This is additive protocol support compared with the prior convenience-server
feature selection, which did not enable Hyper HTTP/2. It adds the `h2` dependency
closure to HTTP-capable consumers; it does not add security/JWKS to web-only or
Axum to configuration/core-only consumers. Protocol and dependency isolation
checks are separate verification requirements.


String-list source coverage additionally uses
`scafra-config/tests/operational.rs::supported_string_lists_work_from_all_sources_and_reject_invalid_elements`
for all supported list properties, including property/environment/programmatic
flow-sequence syntax. Sequence leaves are allowed only where the serialized
schema declares a list; positional struct sequences and mappings cannot bypass
framework path validation. These guards preserve feature checks and redacted
errors for malformed list elements.

Managed upgraded IO retains admission permits until release. After protocol
handoff the application owns session inactivity, streaming limits and shutdown;
the HTTP watchdog does not claim to terminate WebSocket work. HTTP/2 request
futures are different: their connection-local tracked executor aborts and joins
them before forced drain returns. The 32-iteration, two-worker regression
`forced_http2_drain_cancels_pending_handler_before_returning` demonstrated and
then verified correction of the original detached-stream cleanup race.

The final name audit also found that valid mixed-case/dotted/double-underscore
scheduler identifiers were not uniformly addressable. Framework-only literal
task path handling and reversible `TASK_HEX_` environment segments now preserve
all supported names, reject ambiguity/duplicates and retain domain-loader
isolation. `scheduler_names.rs` covers all sources and real 2 ms execution
against a 60000 ms registration default. Late profile selector environment or
ordinary overrides now fail with guidance rather than being accepted after
profile resolution. Incomplete Basic credentials alongside a usable bearer
authenticator also fail validation instead of silently disabling that scheme.

Final generated-runtime coverage is preserved by
[scripts/issue111-runtime.py](../../scripts/issue111-runtime.py): file formats,
profiles, environment, raw chunked body/timeout/header behavior, banner/config
display/logging, effective BootUI profile and safe snapshot/authentication,
joined dashboard shutdown, and a genuinely HTTP-free scheduler consumer.

Final frozen source verification: all seven workspace commands and the Rust1.88
locked all-feature check pass; suites report343default/471all-feature tests.
The separately executed external/MSRV catalogue now passes16/16consumers on
Rust1.88; the negative suite passes9/9 after migrating two outdated RouterOptions
fixtures; staged verification passes all12packages and2registry-source consumers.
The task owner authorized the exact output subtree on2026-10-25. These outcomes
are independently recorded and are not inferred from workspace success. See the
[final review](cfg-01-final-review.md) for commands, logs, preserved failed attempt,
unchanged production-source fingerprint and final independent review.

## Critical reassessment follow-up

Three gaps were reproduced after the first completed audit. They did not add
new configuration keys or change the inventory counts:

| Finding | Correction and effective consumer | Regression evidence |
| --- | --- | --- |
| Programmatic overrides were not injectable into standard startup | `run_with_loader` / `run_with_loader_and_resources` accept the authoritative loader; `#[scafra::main(config_loader = path)]` supplies it once. The existing facade capability mask still narrows the input. Address, logging, graph, scheduler, BootUI profile and shutdown share its validated result. | [startup_loader.rs](../../crates/scafra/tests/startup_loader.rs), [generated runtime script](../../scripts/issue111-startup.py), macro parser/compile-fail tests |
| Bind failure waited on a scheduled callback before requesting its dependent resource cancellation | Standard runner requests resource cancellation immediately after the serving result, before any cleanup join. Scheduler-start failure also cancels before lifecycle cleanup. | [startup_bind_cleanup.rs](../../crates/scafra/tests/startup_bind_cleanup.rs), actual occupied loopback port with an active dependent job |
| `ConfigLoader` derived Debug exposed programmatic secrets | Manual Debug omits all override keys/values regardless of field name; `Properties<T>` inherits that redaction. This is an X15-style fixed redaction invariant, not a configurable exception. | [debug_redaction.rs](../../crates/scafra-config/tests/debug_redaction.rs), ordinary/pretty/cloned/nested diagnostics |

No framework-wide process shutdown deadline is claimed: reverse resource
cleanup awaits user-owned callbacks, whose `ShutdownPolicy` is advisory. Managed
HTTP enforces its own configured drain deadline. Cancellation cannot preempt
non-cooperative synchronous callbacks.
