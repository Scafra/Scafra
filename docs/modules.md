# Modular Scafra and migration

Scafra uses one workspace, independently published Cargo crates, and explicit
facade features. Default features are empty. These APIs are implemented at the
current workspace version `0.1.0`; the architecture targets 1.0 without claiming
that 1.0 has been published.

## Cargo feature reference

| Facade feature | Enables | Excludes unless selected elsewhere |
| --- | --- | --- |
| default / none | Core, foundation, generic typed config, component/config macros | Axum/HTTP, security, actuator, scheduler, JWT/JWKS, BootUI |
| `web` | Axum/Tower routing, JSON, request limits/tracing, HTTP runner | Security, actuator, scheduler, JWT/JWKS |
| `security` | `web`, Basic/static bearer and policy enforcement, config adapter | JWT/JWKS |
| `jwt` | `security`, jsonwebtoken and URL parsing for local verification | Remote client/discovery |
| `jwks` | `jwt`, reqwest remote keys and issuer discovery | Other integrations |
| `actuator` | `web`, health/live/ready/info/metrics and config adapter | Application security, JWT/JWKS |
| `scheduler` | Scheduler and config adapter, Tokio | HTTP/Axum |
| `bootui` | `security`, dashboard and config adapter | Actuator, scheduler, JWT/JWKS |
| `standard` | web, security, jwt, jwks, actuator, scheduler | BootUI |

Features are additive unions. `default-features = false` on one dependency
cannot remove a feature another dependency enables. Use direct `scafra-core`
or `scafra-config` when the facade is unnecessary. They do not depend on HTTP.
Each optional module is absent from a minimal consumer's **normal dependency
closure**, not merely hidden from Rust exports. Macro host dependencies still
cost compilation time and are not application runtime services.

```toml
# Minimal REST API
[dependencies]
scafra = { version = "0.1", default-features = false, features = ["web"] }
```

For a protected API replace `web` with `security` (Basic/static bearer), `jwt`
(local token verification), or `jwks` (remote keys/discovery). Enable credentials
in configuration; a Cargo feature does not authenticate any request by itself.
For health endpoints use `["web", "actuator"]`; explicitly select endpoints and
configure their independent bearer guard as appropriate. For an HTTP-free
scheduler/core consumer use `["scheduler"]` and an application-owned Tokio
entrypoint. A full compatibility application selects `["standard"]`, adding
`bootui` only if needed.

## Dependency direction and ownership

Foundation owns logging, shutdown policy, phases, and HTTP-independent policy
declarations. Core owns typed graph/lifecycle/resource contracts. Config owns
the single generic loader and feature-selected aggregate. Security and web share
policy declarations from foundation; web's security and actuator dependencies
are optional. This avoids dependency cycles and preserves integrated web APIs
when the corresponding features are selected. No reflection, runtime scanning,
TypeId/Any container or service locator is used.

`scafra-config` defaults are empty; its `security`, `jwt`, `jwks`, `actuator`,
`scheduler`, `bootui` features control integration fields and validation.
`standard` selects all except BootUI. `scafra-security` defaults are empty;
`jwt` selects local verification, `jwks` adds remote keys/discovery, and
`standard` selects both. `scafra-web` defaults are empty; its `security`,
`actuator`, `jwt`, `jwks`, `standard` features enable the corresponding adapters.

## Configuration and fail-closed security

`ConfigLoader`, `Properties<T>`, profiles, source selection, required explicit
paths, environment schemas, redacted errors, and precedence remain available
without integrations. Precedence remains defaults → base files → profile files
→ environment → explicit overrides. Generic properties retain #78's prefixed,
explicit double-underscore and unambiguous legacy environment names.

Framework root namespaces (`server`, `logging`, `startup`, `security`, `actuator`,
`scheduler`, `bootui`, `scafra`) are reserved. Unknown framework fields and
explicitly configured unavailable sections fail even if `enabled: false` or a
later override would erase the section. Custom application namespaces remain
valid. This applies to selected files/profiles, environment and explicit
loader overrides. Integration-specific environment schema adapters are compiled
only with their corresponding config features; there is still one parser.

The standard runner narrows `ConfigLoader` using `ConfigCapabilities` derived
from **its own selected facade features**. This matters when another dependency
enables more config/web features through Cargo unification. Explicit global
security cannot be accepted and then silently omitted by the runner. Custom
runners can use `.capabilities(ConfigCapabilities { ... })` similarly; the mask
only narrows compiled capability availability.

Authorization metadata is always available to web. Protected routes with no
security implementation or disabled security fail router construction before
serving. Controller/route public overrides retain their precedence. Roles and
scopes require enforcement even on handwritten inherited policies. Selecting
security without JWT cannot accept JWT intent. Selecting JWT without JWKS
supports a local secret plus issuer/audience restrictions, but rejects remote
key/discovery intent. Verification failure never falls back to Basic/static
bearer. Signature/algorithm/issuer/audience/cache checks remain implemented by
the existing verifier. Startup diagnostics are bounded and print safe fixed
hints even before logging initialization; they do not dump arbitrary payloads.

Actuator remains independent of application security. Empty endpoint selection
adds no endpoints. Its own bearer policy is enforced when selected; global
security can provide an additional layer. Unknown readiness-check names reject
fallible standard startup; the legacy infallible actuator router reports DOWN.

## Lifecycle, generated code and workers

`Application`, `ApplicationContext`, typed providers, graph errors,
`AsyncResource`/`AsyncResourceSet`, shutdown reason and policy retain their
contracts. Providers initialize in graph order. Partial failures unwind completed
resources in reverse order. Resources remain owned while HTTP requests and
scheduler tasks run. Shutdown signals cancellation before HTTP drain, joins the
scheduler, shuts down hooks, then shuts down resources. Original failures retain
precedence over cleanup errors. Cancellation callbacks must remain idempotent.

`#[scafra::main]` is an HTTP entrypoint requiring `web`; controller/routes also
require `web` and produce feature diagnostics when omitted. Service, bean,
Config derive, and core graph composition work without HTTP. Generated optional
items use facade-owned token adapters selected by dependency features, never
`cfg(feature="web")` in the application's unrelated feature namespace.

Graphs expose `into_resources()` independently of HTTP. With web, use
`build_router_with_options_and_resources(&RouterOptions)` and retain/clean up
its returned resource owner even when the router result fails. Existing
integrated graph helpers remain available when actuator and security are both
selected. Do not use a router-only helper to drop managed async resources early.

HTTP-free core hooks and scheduler consumption are supported. A supervised
production worker entrypoint remains #49, outside this implementation. BootUI
retains its existing task model: a post-bind extension task served on its own
listener, ending when the owning runtime ends. It is not a second application
lifecycle owner or a promise of separately graceful dashboard draining.
BootUI loopback mode requires real `ConnectInfo<SocketAddr>`; an in-memory Tower
test must supply that metadata rather than expecting a peerless request to pass.
Dashboard snapshots remain an explicit allow-list of server/profile data.

## API migration from the previous facade

- Replace bare `scafra = "0.1"` with explicit features. `standard` retains the
  previously available runtime capability set. Add `bootui` for the dashboard.
- Integration fields on `ScafraConfig`, module payload variants on `StartupError`,
  root/prelude exports, and HTTP `scafra::Result` are feature-dependent. Update
  struct literals and exhaustive matches to match the application's selection.
- Direct config/security/web consumers select their own features; facade
  selection forwards them automatically. The config aggregate and configured
  extension registration retain their paths to avoid a facade/BootUI cycle.
- `AuthorizationMode`, `AuthorizationPolicy` and inventory metadata have one
  shared identity in foundation and retain web/security/facade re-export paths.
- The options-based web API is available with web alone: `RouterOptions`,
  `build_router_with_options`, `finish_router_with_options`,
  `validate_routes_with_options`, `operational_router`, and
  `serve_router_with_options_and_callbacks`. Integrated legacy builders retain
  their signatures when their named capabilities are enabled.
- Keep `__private` generated paths compatible within coordinated package
  releases. The facade source split keeps root re-exports; source organization
  alone does not provide dependency isolation (#82).

## CLI and verification

All four application kinds generate minimal web manifests, including `service`
(which still demonstrates HTTP health, not a worker). Select additions simply:

```bash
scafra new my-api --kind api --features security,actuator
scafra new full-app --features standard,bootui
```

Security selections generate enabled security config requiring supplied
credentials; no sample secret or fallback bypass is installed. Generated
manifests use explicit defaults/features in local and registry modes.

```bash
cargo fetch --locked
python3 scripts/arch103-consumers.py /var/tmp/scafra-consumers-NEW --runtime
python3 scripts/arch103-negative.py --output /var/tmp/scafra-negatives-NEW
python3 scripts/arch103-publish-order.py --check
python3 scripts/arch103-package-check.py /var/tmp/scafra-packages-NEW --offline
```

Each consumer resolves independently outside the workspace. Tests cover normal
package absence, real loopback JSON/auth/readiness, SIGTERM, resource cleanup,
HTTP-free scheduler drain, missing features and Cargo feature union. CI retains
workspace checks and runs the isolated MSRV matrix. The declared MSRV remains
1.99.1. Verified target policy is Linux x86_64; additional targets require their
own checks before a support claim. Package staging verifies normalized archives
against a local directory registry source. Actual crates.io propagation and
credentials remain release operations, not something this task performs.

See [implementation results](architecture/arch-01/implementation.md) for exact
commands, measurements, acceptance status and remaining verification limits.
The Phase 0 baseline files remain historical before-refactor evidence.
