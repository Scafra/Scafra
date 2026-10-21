# Scafra configuration

The default feature set is empty. `ConfigLoader`, typed `Properties<T>`,
`Config`, source/profile resolution, and the generic server, startup, and logging
models do not depend on HTTP, security, scheduling, or actuator implementations.

`ScafraConfig` keeps its existing public path and composes module-owned models
through additive features:

| Feature | Aggregate field/capability |
| --- | --- |
| `security` | `security`, Basic authentication and static bearer configuration |
| `jwt` | `security` and local JWT verification |
| `jwks` | `jwt` and remote issuer/JWKS configuration |
| `actuator` | `actuator` |
| `scheduler` | `scheduler` |
| `bootui` | `bootui` (data-only dashboard configuration) |
| `standard` | security, JWT/JWKS, actuator, scheduler; excludes BootUI |

Existing direct config consumers can select `standard`, adding `bootui` if
needed. Public integration fields and re-exports require their corresponding
features. `BootUiConfig` remains available as a data-only model without the
BootUI feature; the feature selects its aggregate field and namespace.

The loader reserves `server`, `logging`, `startup`, `security`, `actuator`,
`scheduler`, `bootui`, and `scafra` namespaces, including when loading custom
properties. Other application namespaces remain allowed. Selected YAML and
properties inputs are checked **before merging**, including profile inputs.
An explicitly configured unavailable module is an error even if its section is
empty, null, or disabled, or later overwritten. Unknown keys within framework
namespaces are errors. Environment variables and explicit override paths are
checked against the same reserved contract. This prevents a missing module or
misspelled policy from silently dropping security requirements.

Module adapters provide feature-gated schema information for deserialization-only
secret fields and dynamic task entries. Generic environment path resolution still
uses the typed default schema, qualified by the `ConfigProperties` prefix; file
keys never broaden it. The existing precedence remains defaults, conventional
base files (YAML, YML, properties), selected profile files in that order, sorted
environment variables, then explicit overrides. Explicit source failures do not
fall back to another directory or file. Validation errors expose only approved
safe field/reason details through `validation_details()`.

Cargo feature unification can compile configuration adapters for more modules
than a particular facade/runner selects. Runners must call
`ConfigLoader::capabilities(ConfigCapabilities { ... })` with their own compiled
capability flags. The loader intersects this mask with its compiled availability;
repeated calls can only narrow it. Module configuration in any selected source,
profile, environment variable or override is rejected when the runner mask lacks
that capability. Serialized aggregate defaults do not count as explicit module
selection. A local JWT verifier may use `issuer_uri` and audiences without JWKS;
issuer-only discovery and `jwk_set_uri` require the remote-key capability.
