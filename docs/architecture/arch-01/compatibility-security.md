# Compatibility, security and lifecycle review

> Historical Phase 0 snapshot/planning evidence. The [implemented contract](../../modules.md)
> and [implementation results](implementation.md) supersede proposed/current-state claims below.

This is a proposed contract at Phase 0, not a claim that optional builds exist.
Read alongside [ADR-001](adr-001-modular-framework.md).

## Public API review and migrations

| Current public surface | Proposed handling | Compatibility consequence / migration |
| --- | --- | --- |
| Root and `prelude::*` exports in `scafra/src/lib.rs` | Gate HTTP, auth, actuator, scheduler exports by selected capability; preserve core/config/component macros | Bare facade dependency loses implicit modules if empty defaults approved; initially add `features = ["standard"]` |
| `ScafraConfig` fields `server`, `logging`, `startup`, `security`, `actuator`, `scheduler`, `bootui` | Keep aggregate in config; optional integration fields and validation | Integration field access/struct literals require matching features. Retain neutral config models; direct config callers opt into adapters |
| `scafra::config` and direct config re-exports of Security/Jwt/SchedulerConfig | Conditional same paths as aliases of module-owned types | Direct generic consumers keep ConfigLoader/Properties API; integration consumers select capabilities explicitly |
| `Config::validation_details` and `ValidationError` | Keep generic safe contract and default method | Avoid required trait additions; schema metadata should use an additive API/default |
| `StartupError::{Config,Address,Core,Graph,Scheduler,Web}` / `kind()` | Gate module-specific payload variants; preserve existing labels for selected variants | Exhaustive matches change per feature set; document pre-1.0 break. No stringification of hidden payloads; add capability error if needed with deliberate taxonomy review |
| `scafra::Result<T> = Result<T, AppError>` | Keep HTTP alias under web | Core-only code uses explicit `std::result::Result`; do not silently change error meaning |
| `run`, `run_with_router`, `run_with_resources` and main | Keep HTTP signatures under web, delegate selected integrations to same lifecycle owner | No new worker semantics; choose web for existing generated applications |
| `ControllerPrefix::AUTHORIZATION_POLICY`, web auth re-exports and security auth types | Move definitions/inventory identity to web and re-export from security | Same public paths/type identity where possible; no duplicate inventory collections or structurally similar replacement types |
| `web::finish_router`, `validate_routes`, long serve/build functions taking Actuator/Security configs | Introduce neutral route/server primitives and facade integration wrappers | Direct web callers need migration to neutral primitives + explicit layers, or facade wrappers. Keeping signatures that depend on security while security depends on web is cyclic |
| `SecurityConfig`, `JwtConfig`, Basic config and layers | Keep typed config available for validation, gate executable JWT/JWKS paths | Missing code produces safe validation failure; callers selecting JWT features keep old behavior |
| Actuator `router`, `try_router`, `router_with_health_registry` | Preserve direct package functions and fallback semantics | Legacy router stays DOWN on unknown selection; standard path remains fallible |
| `Application`, context, hooks, graph, AsyncResource/Set, shutdown policy/reason | Preserve signatures, typed ownership, deterministic graph behavior | No replacement container, trait blanket impl, Tokio requirement in core, or resource lifetime shrinkage |
| `__private` logging/runtime/supervisor paths and generated graph methods | Keep existing selected standard path and add hidden feature adapters | Hidden paths are generated-code compatibility; test graph/build-script and main consumers together |
| ConfiguredExtensionRegistration and BootUI manual startup | Keep same effective aggregate/profile semantics | BootUI selects required config/security features. Do not cause duplicate extension registration by adding a facade re-export |

`standard` is a capability migration aid; it does not exempt applications from
new unknown-framework-section checks. Deprecation aliases are possible for
facade paths. Cross-crate cycles make some direct web signatures impossible to
retain under the proposed ownership; explicitly release-note these before 1.0.
No cross-feature type layout/ABI promise is made. Do not publish independently
incompatible macro/facade versions; retain compatible coordinated versions and
prove registry resolution. A facade file split under #82 alone changes none of
these dependency constraints.

## Security review: required negative proofs

Each case must run in an isolated external dependency graph with features
specified by the application manifest. Compile-only checks are insufficient for
startup/config/policy behavior. The following are implementation acceptance
tests; Phase 0 does not pretend they have already been implemented.

| Risk | Required negative test and expected result |
| --- | --- |
| Middleware removed but route metadata discarded | Web-only app with authenticated controller/route, roles or scopes: actionable compile error or router-construction error before bind; never 200 anonymous |
| Runtime-disabled enforcement | Secure build with `security.enabled=false` and inherited protected route: startup fails; explicitly public route semantics retained when the whole app's effective protected routes are absent |
| Handwritten route adapters bypass metadata | Manually supplied protected policy records with no enforcement: constructor rejects; conflicting, duplicate or missing records cannot override an explicit protection declaration |
| Global permit rules broaden protection | Broad permit_all plus explicit authenticated route, method/HEAD, trailing slash, wildcard and overlapping routes: explicit protection remains enforced |
| Missing JWT support | `security.jwt.enabled=true` with security-only build from file/profile/environment/override: fails before listener; static bearer credentials do not provide a fallback |
| Missing JWKS support | jwt build with issuer_uri or jwk_set_uri: startup fails; no requests accepted as authenticated using unverified claims |
| JWT verification regresses | Wrong signature/algorithm/issuer/audience, expired token, malformed or duplicate kid, expired cache with refresh failure: unauthorized; retain refresh/timeout/rotation tests |
| Missing compiled module ignored by Serde | Omitted security/actuator/scheduler/BootUI section from selected base/profile files or env/override: safe capability error, including explicit disabled sections and sections later overwritten with null |
| Unknown framework keys become defaults | Misspelled `security` child key, JWT verifier option or actuator token key: reject with key/source context; custom `mail`/application properties remain valid |
| Environment schema accidentally broadens | File-only unknown key plus env override: remains rejected; nested/dotted properties, ambiguous single underscores and duplicate explicit/legacy paths preserve #78 behavior |
| Invalid source falls back | Missing explicit file/dir, unsupported extension, malformed YAML/properties, conflicting selectors, invalid profile: error, never implicit fallback to unsecured defaults |
| Secret leakage | Sentinel token/password/URI/provider message in parse/validation/startup/cleanup error: Display, Debug, structured logs and BootUI snapshot omit values; field/source names remain actionable |
| Actuator exposed unintentionally | No actuator feature: no operational routes. Feature selected but endpoints empty: none. Token enabled but absent/blank: startup fails; wrong/no token unauthorized when endpoints selected |
| Readiness typo grants readiness | Unknown selected check in legacy/fallible and graph-scoped modes: legacy DOWN; standard startup failure. Independent app registries never see each other's checks |

Current security uses redacted Debug implementations and skipped secret
serialization in selected fields. Schema extraction cannot rely only on
serialized defaults, because some secrets are deliberately absent. Avoid
dumping merged raw configuration or inherited environment into baseline logs.
The synthetic baseline token is a fixed test value, not a credential.

## Lifecycle invariants and fault injection

Use the existing tests/fixtures as the oracle before extracting runner logic:

1. Config resolution/validation fails before providers or listeners run.
2. A provider failing at each graph position rolls back only completed owned
   resources in reverse dependency order; failed provider futures are not
   arbitrarily cancelled. Cleanup failures attach without hiding the original.
3. External output providers remain ordinary functions; no orphan Bean impl or
   global state becomes necessary. Arc injection preserves owner access.
4. Router metadata/readiness failures after async composition return resources
   to the failure cleanup owner. Hook startup rollback and scheduler start
   failures each clean up exactly once.
5. Listener bind/signal failures trigger scheduler/resource cancellation and
   cleanup with the correct ShutdownReason; no post-bind extension starts on a
   bind failure. Keep effective config/profile passed to extensions.
6. Shutdown signals scheduler/resources before active HTTP request drain;
   scheduler drains before hooks, then resources close in reverse order.
   Forced HTTP timeout respects ShutdownPolicy, original error wins, cleanup
   errors remain observable through safe metadata.
7. Core-only hooks and scheduler-only dependency builds have no HTTP owner.
   Feature gating must not introduce a second lifecycle owner or promise #49
   supervision. Tokio runtime teardown remains distinct from explicit shutdown.

BootUI currently detaches its server task and logs policy/bind failures. Turning
it into an owned managed resource is a separate scoped follow-up if maintainers
require it; preserve current behavior and record this limitation while isolating
dependencies. Debug reload currently force-kills its child; do not conflate that
behavior with standard graceful shutdown.

## Implemented route-local correction

The Phase0 ownership proposal above is historical. Foundation retains neutral
policy declarations; generated MethodRouter guards now enforce concrete metadata.
New `finish_managed_router`/`FinalizedRouter` and finalized runner/resource APIs
support nested public overrides without re-finalization. Legacy raw APIs retain
global enforcement. Managed composition explicitly guards handwritten routes
before merging; generated graph inventory adapters have an isolated local legacy
security boundary. See [current review](final-review.md) and [migration](../../modules.md).
