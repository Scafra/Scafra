# Configuration reference

This reference describes implemented settings in the 0.1.0 workspace. It is
the canonical key/default reference for CFG-01; it does not make the proposed
1.0 API policy a shipped guarantee. The [source audit](architecture/configurability-audit.md)
records consumers, exceptions and test coverage. Actual command outcomes are
recorded separately in the [verification report](architecture/cfg-01-final-review.md).

## Sources, precedence and validation

Use `ConfigLoader::load_validated::<ScafraConfig>()`. The standard runner does
this before building application resources or binding its listener. Values are
resolved once at startup; changing a file or environment value requires a
restart, without recompilation. This is not a live configuration service.

For every ordinary runtime property below, supported sources are **YAML, YML,
properties, profile files, environment and programmatic overrides**. Precedence
is typed defaults → `application.yaml` → `application.yml` →
`application.properties` → the same three extensions for the active profile →
environment → `ConfigLoader::override_value`. Later sources win. Explicitly
selecting a base file replaces the base-file search, while profile files still
come from that file's directory. A selected file/directory must exist; missing
conventional files inside a valid directory are optional.

Use double underscores between path components and preserve underscores within
a field: `server.max_request_body_bytes` becomes
`SCAFRA_SERVER__MAX_REQUEST_BODY_BYTES`. The prefix is followed by **one**
underscore. Legacy single-underscore names work only when the serialized schema
identifies exactly one path. Duplicate environment spellings resolving to one
path, ambiguous names, invalid names and unknown fields fail. Secret fields
omitted by serialization remain supported injection targets. Sequence values
can use YAML flow syntax, for example `security.jwt.audiences=[api]` in a
properties file. Only schema-declared list leaves accept sequences; mappings
or positional sequences cannot replace framework configuration structs.

Framework namespaces are checked against compiled capabilities before merging;
later overrides cannot erase unavailable security intent. Optional sections
require their feature even if `enabled: false`. Domain namespaces remain
available for application `Properties<T>`. Diagnostics omit invalid values;
avoid putting credentials in property names or filesystem paths, which are
location metadata. Negative values, integer overflow and invalid scalar types
fail deserialization, then typed validation checks the bounds below. Bounds are
inclusive. `usize` values must also fit the target platform.

`server.request_timeout_seconds` remains an input compatibility alias for
`server.timeout`; serialization uses `timeout`. Aliases are normalized before
merging, so they follow source precedence. Supplying both names in one source
or both programmatic spellings is an error. Use the canonical name in new code.

## Server and shutdown

Types are Rust types with their corresponding Serde representation. A duration
with a `_seconds` or `_ms` suffix is an integer in that unit, not a duration
string. Server keys are always recognized by `scafra-config`; runtime HTTP
consumption requires the facade `web` feature or a standalone `scafra-web`
server. Shutdown settings are independent of HTTP.

| Canonical key | Type | Effective default | Unit; validated range | Runtime owner and operational implications | Valid example; rejected example |
| --- | --- | --- | --- | --- | --- |
| `server.host` | `String` | `127.0.0.1` | Literal IPv4/IPv6 address, validated before composition | Facade constructs listener address. `0.0.0.0` exposes all IPv4 interfaces; no DNS lookup or TLS is implied. | `127.0.0.1`; blank string |
| `server.port` | `u16` | `8080` | TCP port; 1–65535 | Managed listener. An occupied port produces a bind error. Prebound lower-level listeners may use OS-assigned port 0. | `8081`; `0` |
| `server.timeout` | `Option<u64>` / integer or null | `30` | Seconds; 1–86400, or explicit null | Router handler deadline; bounds occupied request work. A missing value retains 30 seconds; null deliberately removes this deadline. | `45`; `0` |
| `server.max_request_body_bytes` | `usize` | `1048576` | Bytes; 1–1073741824 | Global extractor and streaming ceiling. Increasing it increases permissible per-request work; no unlimited setting. | `2048`; `0` |
| `server.header_read_timeout_seconds` | `u64` | `10` | Seconds; 1–3600 | Managed HTTP/1 header-completion deadline, including slow header transmission. | `3`; `0` |
| `server.idle_timeout_seconds` | `u64` | `60` | Seconds; 1–86400 | Managed transport inactivity timer reset by successful read/write progress. It also bounds silent clients before routing; ongoing traffic can keep a connection alive. | `20`; `0` |
| `server.max_connections` | `usize` | `1024` | Concurrent managed connections per listener; 1–1000000 | Connection admission, retaining the permit through a protocol upgrade until its IO is released. At capacity acceptance pauses; the OS backlog remains OS-owned. This is not a per-IP or request-concurrency limit. | `32`; `0` |
| `server.max_header_bytes` | `usize` | `32768` | Bytes; 8192–1048576 | Hyper HTTP/1 parser buffer ceiling and HTTP/2 decoded header-list ceiling. These protocol-specific limits are not identical accounting rules. | `16384`; `8191` |
| `server.http1_keep_alive` | `bool` | `true` | Boolean | Hyper HTTP/1 persistent connections. False trades additional connection setup for shorter connection reuse. | `false`; nonboolean string |
| `server.http1_max_headers` | `usize` | `100` | Header fields; 1–1000 | Hyper HTTP/1 parser field-count ceiling, separate from byte accounting. | `50`; `0` |
| `server.http2_max_concurrent_streams` | `u32` | `200` | Concurrent streams per connection; 1–10000 | Hyper HTTP/2 advertised/enforced per-connection stream concurrency; bounds multiplexed request work independently of TCP connection count. | `80`; `0` |
| `shutdown.grace_period_seconds` | `u64` | `30` | Seconds; 1–86400 | Converts to `ShutdownPolicy` for HTTP drain, lifecycle and graph resource cleanup. Cooperative async work is bounded where the executor applies the policy. | `5`; `0` |
| `shutdown.force_after_grace` | `bool` | `true` | Boolean | False deliberately permits unbounded waiting by policy-aware cleanup. True stops waiting/aborts owned cooperative work; it cannot terminate synchronous code or reverse side effects. | `false`; nonboolean string |

The 30-second processing default gives ordinary request/response services a
finite request occupancy budget consistent with the existing 30-second default
drain budget. A single request cannot keep the default server busy indefinitely.
This is a production guard, not a promise that every workload finishes within
30 seconds. Disabling the processing timer leaves transport inactivity active;
long quiet operations must also choose an adequate inactivity budget. These
per-request and per-connection limits are not a whole-process memory quota.
HTTP/2 stream concurrency multiplies by admitted connections; size both for the
application's memory and workload budget. Choose a larger explicit budget for long operations or explicitly
disable it when the embedding deployment supplies a suitable policy.

The deadline uses deadline-first tie behavior and drops the handler future on
expiry, returning HTTP 504 with `request_timeout`. It does not cover response
body streaming after headers, preempt blocking synchronous code, cancel detached
background work, guarantee cancellation of external operations, or roll back
external side effects. Independently configure clients/databases for their own
deadlines. Resource and scheduler cancellation requests similarly do not promise
that a synchronous callback has stopped. See the lifecycle discussion in the
[design](architecture/cfg-01-design.md). A grace period applies to each
policy-aware drain/cleanup wait, not an advertised whole-process wall-clock
shutdown SLA.

### Streaming bodies and route overrides

The framework uses byte accounting while bodies are polled; it does not buffer
the full request or read ahead. A trustworthy over-limit size hint or declared
`Content-Length` can reject early. Actual bytes are still counted regardless of
the declared length. Hyper owns HTTP framing: bytes beyond a shorter
`Content-Length` are not part of that same request body.

If a handler encounters the ceiling before returning its response, the outer
guard returns HTTP 413 and `application/json`, even if the handler swallowed the
body error:

```json
{"code":"payload_too_large","message":"request body too large"}
```

An unconsumed unknown-length body cannot be classified as oversized without
reading it. A handler that ignores the body or stops before crossing the ceiling
can respond normally. Excess bytes are never delivered through the limited
body. If the request body is forwarded as a response stream, headers can already
be committed when the limit is encountered: the stream fails, and its existing
200 status cannot become 413. Do not treat this as a universal pre-response 413
guarantee. Applications returning success before validating input own that
decision. This boundary is exercised by `unread_streams_and_response_commit_have_explicit_boundaries`.

`scafra_web::with_request_body_limit(router, bytes)` installs a typed, validated
route/router ceiling. A local ceiling can tighten the global ceiling; raising a
local value or disabling an Axum extractor limit cannot remove the enclosing
ceiling. `DefaultBodyLimit` remains available for extractor-specific tightening.
`scafra_web::with_request_timeout(router, Some(duration))` similarly tightens the
enclosing processing deadline. A longer local value or local `None` leaves an
outer deadline effective. To allow a genuinely longer route, choose an adequate
global budget explicitly; router layering cannot silently weaken it. Connection
policies belong to the listener, not individual routes.

### Server ownership

The managed adapter uses Hyper's supported HTTP/1 and HTTP/2 connection builder;
it does not introduce a second parser. HTTP/2 support is additive relative to the
previous convenience-server feature selection; the normal web closure now
includes `h2` and its transport dependencies. `FinalizedRouter` preserves its connection
options, including through nesting, and managed finalized serving uses them.
`into_router()` exposes an ordinary Axum router and therefore transfers connection
ownership to the caller. A user-owned Axum/Hyper server must configure its own
header, inactivity, connection admission and shutdown policies; middleware alone
cannot enforce those controls. Raw-router Scafra serving applies its supplied
`RouterOptions`. Use finalized serving to avoid reapplying middleware.

The header deadline is **HTTP/1-only**. There is no separate HTTP/2 absolute
header-block deadline, HTTP/2 PING keepalive policy, absolute connection lifetime,
per-IP admission policy, TLS configuration, or proxy timeout property. Transport
inactivity and header-list bounds still apply to managed HTTP/2 connections.
Reverse proxies/load balancers own additional ingress limits and TLS, and must
be configured independently. BootUI has its own optional managed listener (see
below); it inherits the same application `server.*` budgets. Admission is
per-listener, so enabling BootUI does not turn the connection count into a
process-wide aggregate ceiling.

After an HTTP upgrade (for example an application-owned WebSocket), the
application owns the upgraded protocol's inactivity and shutdown lifecycle.
The IO retains its connection-admission permit until released, so upgrades
cannot create unlimited admitted sockets. HTTP request deadlines/body accounting
and the HTTP driver watchdog do not become WebSocket/frame policies. Keep owned
session tasks and explicit protocol limits in application code.

HTTP/2 handler futures are tracked in a connection-local executor. Forced drain
closes the driver and aborts/joins its stream tasks before managed serving
returns. A cancellation request cannot preempt non-cooperative blocking code.

## Logging and startup

| Canonical key | Type | Default | Unit; validated values | Runtime owner and implications | Valid example; rejected example |
| --- | --- | --- | --- | --- | --- |
| `logging.level` | `LogLevel` / string | `info` | `off`, `error`, `warn`, `info`, `debug`, `trace`; compatibility aliases `none`, `warning` | Foundation logging initialization. `RUST_LOG` takes precedence for advanced tracing directives. More verbose levels increase diagnostic volume. | `warn`; `verbose` |
| `logging.backtrace` | `BacktraceMode` / string | `off` | `off`, `errors`; aliases `none`, `false`, `error`, `on`, `true` as strings | First logging initialization sets the process backtrace capture policy. Backtraces remain server-side. Capturing them has runtime cost. | `errors`; `always` |
| `startup.banner` | `bool` | `true` | Boolean | Facade startup banner. False suppresses the banner without changing tracing initialization. | `false`; nonboolean string |
| `startup.show_config` | `bool` | `true` | Boolean | Facade banner shows selected safe operational information when true; false shows only name/version. It is not an arbitrary config dump. | `false`; nonboolean string |

Initialization uses `try_init` and does not replace an installed global
subscriber. Applications wanting a different structured format can install
their own tracing subscriber before running Scafra. A `logging.format` setting
is not supported. `RUST_LOG` accepts tracing directives; invalid directives fall
back to the configured simple level without printing their input. Backtrace
policy takes the first initialized value and is not mutable per request.

## Security and JWT/JWKS

The facade `security` feature provides Basic/shared bearer authentication;
`jwt` adds local HS256; `jwks` adds remote RSA verification and implies `jwt`.
Selecting a feature does not turn authentication on. Configure complete
credentials when enabling it. Security and route-policy validation fail closed.
Secret/password values are never included in validation diagnostics or BootUI
snapshots. Do not print arbitrary serialized `SecurityConfig`: the shared bearer
field is serializable for compatibility, although its Debug representation is
redacted. Prefer environment injection for credentials.

| Canonical key | Type | Default | Unit; validation | Runtime owner and implications | Feature | Valid example; rejected example |
| --- | --- | --- | --- | --- | --- | --- |
| `security.enabled` | `bool` | `false` | Boolean; enabling requires a usable authenticator | Security middleware; explicitly protected declarations still require authentication when global security is disabled. | security | `true` with credential; true without authenticator |
| `security.hide_unauthorized` | `bool` | `false` | Boolean | Middleware returns hidden 404 for denied routes when true; does not authorize access. | security | `true`; nonboolean string |
| `security.bearer_token` | `Option<String>` | null | Nonblank if enabled; exact matching | Shared bearer authenticator; treat as a credential, not a role/scoped JWT. | security | injected nonblank token; blank token while enabled |
| `security.basic.username` | `Option<String>` | null | Complete pair needed; configured empty values rejected while enabled | Basic authenticator; use protected transport, and neither Basic nor shared bearer grants roles/scopes. | security | injected nonempty username; empty configured username |
| `security.basic.password` | `Option<String>` | null | Complete pair needed; configured empty values rejected while enabled | Basic authenticator; omitted from serialization and redacted in Debug. | security | injected nonempty password; empty configured password |
| `security.permit_all` | `Vec<String>` / list | `[]` | Exact application paths; no wildcard matcher | Global inherited policy exceptions; cannot remove explicit route/controller role/scope checks. Avoid broader public exposure. | security | `[/public]`; non-list scalar |
| `security.jwt.enabled` | `bool` | `false` | Boolean; requires local secret or remote provider when true | Bearer JWT verification; unavailable verifier intent never falls back to shared bearer/Basic. | jwt | `true` with secret/provider; true without either |
| `security.jwt.secret` | `Option<String>` | null | Nonblank for enabled local JWT; cannot combine with `jwk_set_uri` | Local HS256 verifier; omitted from serialization and redacted in Debug. An issuer restriction can coexist with local secret. | jwt | injected HMAC secret; blank secret |
| `security.jwt.issuer_uri` | `Option<String>` | null | HTTP(S) URL; no credentials/fragment/query in enabled JWT validation | `iss` validation; without a local secret it also selects OIDC discovery. HTTPS recommended outside controlled loopback tests. | jwt; jwks for discovery | `https://issuer.example`; URL with embedded password |
| `security.jwt.jwk_set_uri` | `Option<String>` | null | HTTP(S) URL; no credentials or fragment | Direct remote key endpoint; skips discovery. Configured query strings can be sensitive and are Debug-redacted. | jwks | `https://issuer.example/keys`; `file:///keys` |
| `security.jwt.audiences` | `Vec<String>` / list | `[]` | Enabled JWT rejects blank entries | JWT audience validation. Empty list does not disable signature/expiry validation or the library's audience-claim handling. | jwt | `[api]`; `[""]` |
| `security.jwt.required_scopes` | `Vec<String>` / list | `[]` | Enabled JWT rejects blank entries; every listed scope must be present; an empty requirement list adds no global requirement | Middleware scope checks combine with route-local requirements. Blank requirements fail validated startup. | jwt | `[profile.read]`; `[""]` while JWT enabled |
| `security.jwt.leeway_seconds` | `u64` | `60` | Seconds; 0–300 | JWT `exp` and `nbf` clock-skew tolerance. Zero explicitly disables skew; larger values extend token acceptance around time boundaries. | jwt | `0`; `301` |
| `security.jwt.cache_ttl_seconds` | `u64` | `300` | Seconds; 1–86400 | Router-owned remote key freshness. Larger windows reduce provider traffic and extend cached-key acceptance before refresh. Expired keys fail closed on provider failure. | jwks | `60`; `0` |
| `security.jwt.refresh_cooldown_seconds` | `u64` | `30` | Seconds; 1–3600 and ≤ cache TTL | Router-owned failed/unknown-key refresh throttling. Smaller values increase provider load; larger values delay discovering some rotations. | jwks | `5`; value above TTL |
| `security.jwt.connect_timeout_ms` | `u64` | `2000` | Milliseconds; 1–300000 and ≤ request deadline | Remote client's TCP/TLS connection setup deadline. Prevents stalled transport setup from occupying authentication indefinitely. | jwks | `500`; `0` |
| `security.jwt.request_timeout_ms` | `u64` | `5000` | Milliseconds; 1–300000 and ≥ connect deadline | Entire OIDC discovery plus JWKS retrieval deadline, also supplied to the remote HTTP client. Timeout denies authentication without exposing provider data. | jwks | `3000`; value below connect deadline |

Remote response size (1 MiB), no redirects, signature verification, supported
algorithm constraints, expiration requirement, authorization composition and
secret redaction are fixed security invariants. They do not have opt-out keys.
The four remote policy keys require `jwks` even when explicitly supplied at their
default values or while authentication is disabled. Cargo features cannot be
enabled through configuration.

## Actuator

Requires the facade `actuator` feature or standalone `scafra-actuator`. Actuator
credentials are separate from application credentials; enable protection when
exposing sensitive operational endpoints. Liveness reports process availability;
readiness/aggregate health evaluate the selected checks, without changing those
semantics based on configuration source.

| Canonical key | Type | Default | Unit; validation | Runtime owner and implications | Valid example; rejected example |
| --- | --- | --- | --- | --- | --- |
| `actuator.endpoints` | `EndpointSelection` / list or comma-separated string | `[]` | `health`, `live`, `ready`, `info`, `metrics`, `*`; empty disables all | Actuator router installs only selected groups and aliases. Unknown group values fail validation. | `health,ready`; `heath` |
| `actuator.health.checks` | `Vec<String>` / list | `[]` | Empty or `*` selects all; other names must exist in the graph registry | Health registry selection. Unknown readiness names fail startup or fail DOWN in legacy infallible routing. Checks run sequentially. | `[*]`; unknown registered name |
| `actuator.health.check_timeout_ms` | `u64` | `1000` | Milliseconds per selected check; 1–300000 | Health evaluator drops a timed-out async future and returns DOWN. Total evaluation may consume the sum of per-check budgets. | `500`; `0` |
| `actuator.security.enabled` | `bool` | `false` | Boolean; true requires a nonblank bearer token | Endpoint authentication runs before checks; liveness aliases are protected too. | `true` with token; true without token |
| `actuator.security.bearer_token` | `Option<String>` | null | Nonblank when enabled | Independent shared bearer; omitted from serialization/Debug values. Invalid credentials never run health checks. | injected nonblank token; blank while enabled |

## Scheduler

Requires `scheduler`; it does not require HTTP. Standard web startup forwards
the resolved `SchedulerConfig`; HTTP-free applications call
`Scheduler::try_start(&config.scheduler)` and own their runtime and shutdown.
There is no implicit job registered by enabling the feature.

| Canonical key | Type | Default | Unit; validation | Runtime owner and implications | Valid example; rejected example |
| --- | --- | --- | --- | --- | --- |
| `scheduler.enabled` | `bool` | `false` | Boolean | Scheduler starts no task loops until enabled. Enabled with no linked tasks is a valid no-op. | `true`; nonboolean string |
| `scheduler.tasks.<name>.enabled` | `bool` | `true` | Boolean; name must identify a linked registration | Scheduler task selection. Per-task false suppresses execution regardless of interval. | `false`; unknown task name |
| `scheduler.tasks.<name>.interval_ms` | `Option<u64>` | null: use registration interval | Milliseconds; 1–31536000000 (one year) when supplied | Per-task loop; first run after interval, subsequent runs one interval after callback completion. Null restores the linked baseline. | `5000`; `0` |

Task names are registered at build/link time and must be 1–64 ASCII bytes using
letters, digits, `_`, `-` or `.`. For environment variables prefer names that
remain distinct after the existing case-normalization rules, e.g.
`SCAFRA_SCHEDULER__TASKS__CLEANUP__INTERVAL_MS`. Non-overlap, fixed delay, no queued
catch-up backlog and application-local ownership are the supported scheduling
contract; there are no distributed/persistent scheduling or misfire-mode keys.
Synchronous callbacks run on Tokio's blocking pool and cannot be preempted.

### Literal scheduler names

Task names remain case-sensitive identifiers, including names containing dots
or double underscores. YAML task map keys use the literal name. In properties
and programmatic paths, the final segment is `enabled` or `interval_ms`; the
whole middle is the literal task name. For example:

```properties
scheduler.tasks.worker.tick.interval_ms=2
```

For an ordinary environment name, the task component must identify exactly one
linked registration ignoring ASCII case. To encode punctuation or distinguish
case-colliding names, use `TASK_HEX_` followed by the hexadecimal bytes of the
exact registered name. `worker.tick` is `776f726b65722e7469636b`:

```sh
export SCAFRA_SCHEDULER__TASKS__TASK_HEX_776f726b65722e7469636b__INTERVAL_MS=2
```

This is a reversible environment representation, not a renamed job. Invalid
encoding, ambiguous normal names and duplicate canonical environment targets
fail. Generic domain loaders retain ordinary schema-driven dotted-path behavior;
these task-name rules apply only to framework configuration. See the executed
[scheduler-name matrix](../crates/scafra-config/tests/scheduler_names.rs).

## BootUI

Requires `bootui` (which includes application security support). These are
BootUI's separate dashboard bind settings, not overrides of the application
listener. The standard runner receives the already resolved configuration and
profile and starts an owned dashboard server. It exposes an allowlisted server
host/port/profile snapshot rather than serializing configuration or credentials.

| Canonical key | Type | Default | Unit; validation | Runtime owner and implications | Valid example; rejected example |
| --- | --- | --- | --- | --- | --- |
| `bootui.enabled` | `bool` | `false` | Boolean | Optional dashboard startup; no dashboard listener when false. | `true`; nonboolean string |
| `bootui.host` | `String` | `127.0.0.1` | Literal IPv4/IPv6 when enabled; loopback when local-only; validated before application bind | Dashboard bind resolver. Hostnames are not resolved. | `127.0.0.2`; `localhost` |
| `bootui.port` | `u16` | `8091` | TCP port; 1–65535 | Dashboard listener. Bind failures prevent dashboard startup and are logged. | `8092`; `0` |
| `bootui.path` | `String` | `/bootui` | Non-root static absolute path; rejects control characters, patterns, query/fragment and backslash before application bind | Dashboard mount. Validated config and BootUI preparation reject root or pattern paths. | `/ops`; `/` |
| `bootui.local_only` | `bool` | `true` | Boolean; false requires enabled valid application authentication | Loopback bind and peer checks when true; mandatory dashboard authentication when false. Application `permit_all` cannot make a remote dashboard public. | `false` with auth; false without auth |

`bootui.local_only` is an explicit policy choice, not an authentication bypass.
Validated startup checks enabled BootUI bind/security intent before application
composition; standalone BootUI preparation also checks its policy. Both the
application and dashboard managed listeners consume the same `server.*` request
and connection policies and the same `shutdown` policy. Each listener applies
its own admission ceiling; there is no independent `bootui.timeout` or
`bootui.max_connections` key.

The standard runner owns `BootUiServer`, requests its shutdown when application
shutdown starts, and awaits its drain before releasing application resources.
Standalone applications can call `start_managed_with_config` and retain the
returned owner, use `request_shutdown()` then `shutdown().await`, or drop it to
abort its cooperative async serving task. The compatibility `start_with_config`
entrypoint starts an independent managed server with process-signal shutdown.
`try_layer_with_config` only installs dashboard routes into a caller-owned
server; that embedding server owns transport and drain policies. None of these
paths weakens loopback-peer checks or mandatory remote authentication.

## Profile and source controls

Profile selection occurs before profile sources are read. It is a loader
control, with its own documented ordering; it is not changed retroactively by a
profile document or by a late ordinary property override. Framework environment
`SCAFRA_SCAFRA__PROFILES__ACTIVE` and `override_value("scafra.profiles.active", ...)`
are rejected with guidance to use `SCAFRA_PROFILE` or `.profile(...)`; accepting
these after profile selection would silently ignore their intent.

| Control | Default | Validation and precedence | Consumer |
| --- | --- | --- | --- |
| `scafra.profiles.active` | no selected profile; effective name `default` | String from base document; safe profile name without traversal/path components | Profile source selector |
| `SCAFRA_PROFILE` | absent | Overrides base active profile | Profile source selector |
| `ConfigLoader::profile(...)` | absent | Overrides `SCAFRA_PROFILE`; same name validation | Profile source selector |
| `SCAFRA_CONFIG_DIR` | conventional search in cwd `src/resources` when present, otherwise cwd | Required existing directory; conflicts with `SCAFRA_CONFIG_FILE` | Base/profile source resolution |
| `SCAFRA_CONFIG_FILE` | absent | Required supported YAML/YML/properties file | Exact base-file selection |
| `ConfigLoader::root`, `config_dir`, `file` | default convention | Explicit typed selector overrides environment source selectors; `root` retains the resources convention | Source resolution |
| `ConfigLoader::env_prefix` | `SCAFRA` | Selects property environment prefix; does not rename loader controls such as `SCAFRA_PROFILE` | Schema-aware environment resolver |

## CLI, build and embedding settings

These controls belong to the CLI/compiler or an embedding runtime. They are not
application YAML properties. No `development.*`, `runtime.*`, or Cargo-feature
configuration namespace is implemented.

| Setting | Default | Mechanism and validated values | Owner |
| --- | --- | --- | --- |
| Development watch poll | 500 ms | `scafra dev --poll-interval-ms 750` > `SCAFRA_DEV_POLL_INTERVAL_MS=750` > default; integers 100–60000 ms | CLI supervisor and embedded debug supervisor; changes scan frequency/cost |
| Embedded debug reload | enabled in debug `#[scafra::main]` | `SCAFRA_DEV_RELOAD=false` disables it; CLI uses child marker to avoid nested supervisors | Generated entrypoint |
| Project shape | `web` | `scafra new NAME --kind web/api/service/monolith` | CLI templates |
| Optional capabilities | web-only starter; facade Cargo defaults empty | `scafra new NAME --features ...` generates Cargo feature selection | CLI/Cargo, requires compilation |
| Dev source selection | loader convention | `scafra dev --config-dir DIR` or `--config-file FILE`, mutually exclusive | CLI forwards source selectors |
| Tokio worker/runtime setup | Tokio runtime defaults | Custom main/Tokio builder; Tokio also recognizes `TOKIO_WORKER_THREADS` | Embedding runtime; not a Scafra operational property |
| Build discovery | conventional `src/main` for `#[scafra::main]` | Explicit `scafra_build` source-root APIs, build extension context and Cargo `OUT_DIR` | Build process |
| Controller paths/policies and required typed properties | source declarations | Existing proc-macro arguments and Config derive attributes | Compilation; runtime values still load through ConfigLoader |

## Equivalent examples

This example intentionally chooses a 2 KiB body ceiling for a small-input API.
Choose the body budget appropriate to your application's payloads. The values
below are exercised by `scafra-config/tests/operational.rs`, including all three
file extensions, profile/environment/programmatic precedence and explicit null.
The execution status of that suite is recorded in the verification report.

`application.yaml` (the same content works as `application.yml`):

```yaml
server:
  timeout: 45
  max_request_body_bytes: 2048
  header_read_timeout_seconds: 3
  idle_timeout_seconds: 20
  max_connections: 32
  max_header_bytes: 16384
  http1_keep_alive: false
  http1_max_headers: 50
  http2_max_concurrent_streams: 80
shutdown:
  grace_period_seconds: 5
  force_after_grace: false
```

Equivalent `application.properties`:

```properties
server.timeout=45
server.max_request_body_bytes=2048
server.header_read_timeout_seconds=3
server.idle_timeout_seconds=20
server.max_connections=32
server.max_header_bytes=16384
server.http1_keep_alive=false
server.http1_max_headers=50
server.http2_max_concurrent_streams=80
shutdown.grace_period_seconds=5
shutdown.force_after_grace=false
```

`application-production.yaml`, overriding two values for a smaller-input service:

```yaml
server:
  max_request_body_bytes: 400
shutdown:
  grace_period_seconds: 4
```

Environment overrides use the actual schema-aware names:

```sh
SCAFRA_PROFILE=production \
SCAFRA_SERVER__MAX_REQUEST_BODY_BYTES=700 \
SCAFRA_SHUTDOWN__GRACE_PERIOD_SECONDS=7 \
SCAFRA_SERVER__TIMEOUT=12 \
SCAFRA_DEV_RELOAD=false cargo run
```

Programmatic resolution (requires an existing configuration directory):

```rust
use scafra::{ConfigLoader, ScafraConfig};

let config = ConfigLoader::new()
    .config_dir("src/resources")
    .profile("production")
    .override_value("server.max_request_body_bytes", "800")
    .override_value("shutdown.grace_period_seconds", "8")
    .load_validated::<ScafraConfig>()?;
```

This resolves a typed value; custom composition must pass it to its consumers.
The standard `#[scafra::main]` path already loads and forwards its own final
validated configuration. Do not load a second value and assume the standard
runner automatically adopts it. Lower-level composition can pass
`scafra::router_options(&config)` to router finalization and
`config.shutdown.policy()` to serving/resource cleanup. A supplied finalized
router retains the options it was finalized with.

To explicitly opt out of request processing timeout, use `server.timeout=null`
in properties, `server: {timeout: null}` in YAML/YML,
`SCAFRA_SERVER__TIMEOUT=null`, or `.override_value("server.timeout", "null")`.
`server.timeout=0`, body limit 0, invalid/unknown keys and integer overflow are
rejected; an absent timeout never opts out.
