# CFG-01 configuration and resource protection design

Source baseline: d2bb06b. Branch: `111-cfg-01-audit-all-scafra-defaults-and-make-operational-settings-overridable-before-100`.
The user explicitly authorized proceeding without the unavailable repository skills.

## Verified initial gaps

The existing ConfigLoader already resolves typed defaults, YAML/YML/properties,
profiles, schema-aware environment overrides and explicit overrides. Keep it.
The web router installs extractor-only DefaultBodyLimit(1 MiB); raw bodies are
unbounded. ServerConfig timeout defaults to None. The runner uses the foundation
30-second shutdown default instead of resolved configuration. JWKS cache and
network timing policies are fixed. Actuator check timeout lacks bounds validation.
Both embedded development reload and CLI reload poll every 500 ms.

## Decisions before implementation

Keep module-owned typed settings and immutable startup composition. Preserve
`server.timeout` and normalize its compatibility alias before merging each source.
Absent timeout means 30 seconds; explicit null disables it. Thirty seconds is the
standard interactive-service request budget, aligned with the existing 30-second
HTTP drain budget: ordinary managed requests cannot occupy slots indefinitely or
routinely outlive the default drain window. Long operations must deliberately
increase/disable it, with finite connection and body protection still active.
This is a deployment convention, not a performance measurement or universal SLA.

Keep the default body ceiling at 1 MiB. Use http-body-util Limited for streaming
byte accounting, with a small wrapper recording a limit error so handlers cannot
swallow it and return success before response headers are committed. Reject known
oversize bodies before invoking handlers. Never prebuffer/drain unconsumed bodies.
The ceiling applies to actual bytes consumed, independent of Content-Length.
An ignored/unread body cannot be classified as oversize without reading it;
response streaming after headers commit cannot be changed into a JSON 413.
Document and test these boundaries explicitly. Route layers may tighten global
limits, but cannot increase/disable the global safety ceiling. Axum DefaultBodyLimit
remains supported for extractor-local policies.

Use the existing Hyper stack via hyper-util auto connection builder, rather than
a custom parser or additional HTTP implementation. Retain HTTP/1 and HTTP/2,
ConnectInfo, upgrades, cooperative graceful drain and explicit cancellation.
Expose header-read deadline (HTTP/1), header bytes, HTTP/1 keep-alive, transport
inactivity timeout and maximum concurrent connections. A Tokio IO adapter expires
connections with no successful read/write progress; this is transport inactivity,
not a maximum request lifetime. HTTP/2 preface/header timing remains protocol-owned;
reverse proxies should enforce protocol-specific rate/stream policies. Header/body
and connection admission settings are validated before bind. Connection tasks are
owned and aborted on forced drain; blocking application work remains cooperative.

Defaults: header read 10 s (finite pre-routing budget, tighter than request budget),
transport inactivity 60 s (allows normal keep-alive reuse), 1024 connections (finite
admission ceiling), 32 KiB header budget (ordinary cookies/auth headers with bounded
parser memory), keep-alive true. These are documented deployment conventions;
applications with different traffic/resource budgets override them at startup.

Shutdown settings live in `shutdown`, map to foundation ShutdownPolicy, and reach
normal, startup-failure and generated graph cleanup. A policy cannot safely stop
synchronous hooks or blocking scheduled callbacks. Explicit force=false opts out
of bounded asynchronous draining; it never changes ownership or auth invariants.

JWKS cache TTL/cooldown and network timeouts become validated JwtConfig values.
Cryptographic checks, fail-closed authorization, redaction, no redirects and the
1 MiB remote metadata response ceiling remain invariants. Actuator timeout is
validated both through framework configuration and standalone registry/router APIs.
Development polling belongs to the supervisor, via a validated CLI/environment
control, not application YAML. No mutable registry or extra configuration loader.

## Compatibility

New public struct fields require downstream exhaustive literals to add
`..Default::default()`. Existing paths and serving signatures stay available.
Timeout default and managed connection defaults intentionally tighten pre-1.0
behavior; explicit timeout null retains unlimited processing. FinalizedRouter must
retain its connection settings, so serving cannot silently replace resolved values.
No versions change. See configuration reference and upgrading guide for migration.

## Implementation review corrections

Independent review identified and corrected recoverable accept errors ending
the server, non-fused body errors, per-connection route-map rebuilding, missing
standalone actuator credential validation and authoritative BootUI context
retaining application permit exceptions. The managed dashboard now clears
exceptions at both boundaries and is owned/drained/joined by the runner.

HTTP/2 forced cancellation required executable evidence: a multi-threaded
32-iteration test demonstrated detached stream futures could outlive the
connection driver. A connection-local executor now tracks them in a JoinSet,
reaps completed entries during spawning, closes the driver, then aborts and
joins remaining streams. Grace expiry signals a force command to connection
owners and joins them before returning. This adds per-connection task ownership
and a short mutex during H2 stream spawning, not a global registry or HTTP/1
request lock. No performance improvement is claimed.

Connection admission permits travel with IO on HTTP upgrades until it is
released. Upgraded sessions own their inactivity/shutdown after protocol
handoff; they are not ordinary HTTP handler futures. The HTTP driver watchdog
and response deadline do not promise to stop detached WebSocket tasks.

Properties/environment/programmatic list leaves now accept YAML flow sequences.
Framework sequence input is allowed only at schema-declared list leaves, so
Serde positional struct decoding cannot conceal unavailable feature intent.

## Reassessment: authoritative startup overrides and error cleanup

A subsequent review reproduced a bind-error cleanup cycle: the runner joined a
scheduled callback before sending cancellation to its resource dependency. Stop
scheduler dispatch and notify owned resources before cleanup joins on server
errors, retaining the same
ownership and original-error precedence. Scheduler-start errors also notify
resources before synchronous lifecycle cleanup. Joining remains cooperative;
this does not promise preemption of blocking callbacks.

The standard runner previously always created its own ConfigLoader. Loader-only
programmatic tests did not establish the standard six-source contract. Add two
explicit facade entrypoints: `run_with_loader(loader)` and
`run_with_loader_and_resources(loader, build_application)`, the latter consuming
finalized typed graph composition. Existing entrypoints preserve their signatures
and delegate to the same pipeline with a default loader. The caller's loader is
narrowed to facade capabilities, resolved and validated once, before logging,
listener address, graph/resources, scheduler, dashboard and shutdown policy.

`#[scafra::main(config_loader = crate::configured_loader)]` supplies that same
loader from a synchronous `fn() -> ConfigLoader`. It runs once after the existing
main body and before startup resolution. This small startup hook is justified by
the otherwise missing programmatic standard-entrypoint path; it adds no route
annotations, runtime registry, global state or second configuration snapshot.
Reject unknown/duplicate/malformed options with local macro diagnostics.

ConfigLoader diagnostics omit every override key/value and expose only a count;
Properties diagnostics inherit that safe representation. Neither generic domain
credentials nor framework credentials depend on a secret-name heuristic.

The existing AsyncResource shutdown contract remains advisory: implementations
receive the policy and own their cleanup/cancellation behavior. Managed HTTP
waits enforce their deadline. This is not an end-to-end process deadline for
arbitrary resource callbacks, and the reference must state that distinction.
