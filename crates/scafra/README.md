# Scafra framework

Cargo defaults are empty. This example selects HTTP only. Add `security`,
`jwt`, `jwks`, `actuator`, `scheduler`, or `bootui` explicitly; `standard`
preserves the previous full runtime capability set without BootUI.

The `scafra` package provides Scafra's application-facing Rust
framework. Add it under the dependency key `scafra` to keep the familiar crate
name in source code:

```toml
[dependencies]
scafra = { version = "0.1", default-features = false, features = ["web"] }
```

```rust
use scafra::prelude::*;
```

Scafra is built on Axum, Tokio, Tower, Serde, and `tracing`. It provides typed
component and route macros, configuration loading, application startup, and
optional operational features. See the [repository README](https://github.com/Scafra/Scafra)
for examples, supported behavior, and current limitations.
