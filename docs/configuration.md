# Configuration resolution

## Configuration capability selection

Generic config has no integration defaults. Reserved framework sections must
match compiled capabilities; unknown framework fields and unavailable sections
fail before merge. The standard runner further narrows capabilities to its own
features despite Cargo unification. See [modules](modules.md) for details.


Scafra resolves configuration before application resources, route construction,
or listener startup. A source read, parse, decode, or validation error stops
startup with a typed `ConfigError` wrapped by `StartupError::Config`.

## Default and explicit sources

The default convention uses the process working directory. For compatibility
with generated projects and existing applications, Scafra uses
`src/resources/` when that directory exists; otherwise it uses the working
directory itself. Scafra does not search parent directories or the executable
directory. Convention-based files are optional, so a configuration-free
application can use typed defaults.

This preserves the existing development convention without embedding
`CARGO_MANIFEST_DIR` or assuming the executable's deployment location. An
automatic executable-relative root would vary with symlinks and container
layouts; production deployments should provide an explicit source.

For an application launched from different working directories, select an
exact configuration directory or one exact file:

```rust
use scafra::{ConfigLoader, ScafraConfig};

let config = ConfigLoader::new()
    .config_dir("/etc/orders/config")
    .profile("production")
    .load_validated::<ScafraConfig>()?;

let config = ConfigLoader::new()
    .file("/etc/orders/application.yaml")
    .load_validated::<ScafraConfig>()?;
```

`config_dir` requires an existing directory and reads only its conventional
`application.yaml`, `application.yml`, and `application.properties` files.
Those files remain optional; an existing empty directory means defaults plus
environment variables and explicit overrides. This is the documented policy
for applications that intentionally select an empty configuration directory.
Use no explicit selector for a configuration-free application that does not
need a required directory.

`file` requires an existing regular `.yaml`, `.yml`, or `.properties` file. It
uses only that file as its base configuration source. If a profile is active,
the conventional `application-<profile>.yaml`, `.yml`, and `.properties`
files beside the selected file are then applied. A missing selected file,
invalid path, unreadable file, or malformed contents fail loading; Scafra does
not fall back to defaults or a convention-based directory.

All explicit relative paths are resolved against the process working directory
when loading starts. Absolute paths work independently of the working
directory. The compatibility `root` builder selects an application root and
uses `src/resources/` when present, otherwise that root. New deployments should
prefer `config_dir` when they mean an exact configuration directory.

Migration: `.root(path)` now reports a not-found source error when the selected
application root is missing. Create that root, select an existing exact
directory with `.config_dir(path)`, or use the default loader when the
application intentionally relies on typed defaults without a required source.

Source checks and reads follow filesystem symlinks according to the operating
system. Keep selected configuration paths and their targets under the
deployment's normal filesystem access controls.

The standard runner accepts one of these environment selectors:

```bash
SCAFRA_CONFIG_DIR=/etc/orders/config SCAFRA_PROFILE=production ./orders
SCAFRA_CONFIG_FILE=/etc/orders/application.yaml ./orders
```

Setting both selectors is an error. These variables select the source; the
remaining `SCAFRA_*` variables continue to apply as typed configuration
overrides.

When code selects a source with `ConfigLoader::root`, `config_dir`, or `file`,
that builder selection is authoritative. The `SCAFRA_CONFIG_DIR` and
`SCAFRA_CONFIG_FILE` selectors are consulted only by the default loader used by
the standard runner.

## CLI and generated projects

Generated projects put their default file at
`src/resources/application.yaml`. From the project directory, `cargo run` and
`scafra dev` use that location by convention. The CLI can select an exact
source, and it passes the resolved absolute path to the child process:

```bash
scafra dev --config-dir src/resources
scafra dev --config-file /etc/orders/application.yaml
```

Paths supplied to `scafra dev` are relative to the directory where the CLI was
invoked. It watches the selected source for changes. Direct application
launches from another directory should set `SCAFRA_CONFIG_DIR` or
`SCAFRA_CONFIG_FILE`; Scafra does not embed build-machine manifest paths in the
binary.

For Docker, copy configuration into a stable image or mounted path and select
it explicitly:

```dockerfile
COPY config/ /etc/orders/config/
ENV SCAFRA_CONFIG_DIR=/etc/orders/config
ENV SCAFRA_PROFILE=production
CMD ["/usr/local/bin/orders"]
```

The container's working directory and the executable's directory do not
select configuration when `SCAFRA_CONFIG_DIR` is set.

## Precedence and profiles

Configuration merges in this order:

1. Typed defaults
2. Base YAML, YML, and properties files, in that order
3. Profile YAML, YML, and properties files, in that order
4. Sorted `SCAFRA_*` environment overrides
5. `ConfigLoader::override_value` programmatic overrides

`SCAFRA_PROFILE` selects the profile before files merge and overrides
`scafra.profiles.active` in the base file. `ConfigLoader::profile` is the
explicit programmatic selector and takes precedence over the process
environment. If neither is set, `default` is the active profile. Missing
convention-based base or profile files are optional. Once a file exists, read,
parse, structural, decode, and validation errors stop the load.

For example, a base file can select a profile and provide common limits:

```yaml
scafra:
  profiles:
    active: production
server:
  timeout: 30
security:
  enabled: true
```

`application-production.yaml` can then override production-specific values.
Environment overrides apply after both files, and programmatic overrides take
final precedence. Environment path mapping and validation are described in the
root [README configuration section](../README.md#configuration).

The standard runner loads and validates `ScafraConfig` before initializing
logging, discovering components, creating application resources, or binding
the HTTP listener. Security, request timeout, and resource-limit values are
therefore taken from the same resolved configuration.

## Typed properties

`Properties<T>` uses the same source selection and precedence chain above.
The serialized `T::default()` supplies defaults and the environment path
schema, nested under `#[config(prefix = "...")]`. An empty prefix loads from
the configuration root. Partial files retain defaults for omitted fields.

```rust
use scafra::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Config)]
#[config(prefix = "mail")]
struct MailProperties {
    host: String,
}

// With SCAFRA_MAIL_HOST=smtp.example.test in the process environment:
let mail = Properties::<MailProperties>::new().load()?;
assert_eq!(mail.host, "smtp.example.test");
```

Environment values override base and profile files; an explicit
`.override_value("mail.host", "smtp.internal.test")` on the loader wins last.
Use `.env_prefix("MYAPP")` for a custom environment namespace, such as
`MYAPP_MAIL_HOST`. Nested fields and names containing underscores use the same
schema-aware resolver as `ConfigLoader::load<T>()`. Unknown, ambiguous, and
duplicate environment paths fail loading, and typed deserialization and
`Config::validate` run before properties are returned.

## BootUI

With the standard runner, BootUI receives the already resolved and validated
`ScafraConfig` and active profile after the application listener binds. It does
not load a second configuration copy for its snapshot. `/api/config` exposes
only an allow-listed server address and profile; credentials and unrelated
settings are not serialized.

Custom router composition should pass the same effective value to
`scafra_bootui::try_layer_with_config_and_profile`. The compatibility `layer`
helper uses the standard loader because its signature does not accept the full
application configuration.

Applications that bypass Scafra's standard runner and call lower-level
`scafra_web` server APIs must start BootUI explicitly with
`scafra_bootui::start_with_config(&config, profile)` after the HTTP listener is
ready, or add the dashboard with
`try_layer_with_config_and_profile(router, &config, profile)`. This makes the
configuration handoff explicit and avoids BootUI resolving a second source.
