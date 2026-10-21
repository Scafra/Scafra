# Phase 0 Cargo dependency inventory

> Historical Phase 0 snapshot/planning evidence. The [implemented contract](../../modules.md)
> and [implementation results](implementation.md) supersede proposed/current-state claims below.

Generated from `cargo metadata --format-version 1 --locked` at `e041fc27c46e521e848ecbd3895d85b876c9b831`. This records the manifest contract at that audited commit. At that time all twelve crates declared no Cargo features and no build-dependencies; it does not describe the implemented feature API. A procedural macro's normal dependencies run on the compiler host; they are not application runtime libraries. Target-specific normal edges are retained below. Version requirements are from metadata, resolved versions are in the baseline trees.

| Crate | Direct normal | Build | Dev | Direct Scafra normal dependencies |
| --- | ---: | ---: | ---: | --- |
| scafra | 13 | 0 | 7 | scafra-actuator, scafra-config, scafra-core, scafra-foundation, scafra-macros, scafra-scheduler, scafra-security, scafra-web |
| scafra-actuator | 5 | 0 | 2 | — |
| scafra-bootui | 8 | 0 | 3 | scafra-config, scafra-core, scafra-security |
| scafra-build | 4 | 0 | 0 | scafra-core, scafra-foundation |
| scafra-cli | 5 | 0 | 1 | scafra-foundation |
| scafra-config | 8 | 0 | 0 | scafra-actuator, scafra-foundation, scafra-scheduler, scafra-security |
| scafra-core | 3 | 0 | 0 | scafra-foundation |
| scafra-foundation | 4 | 0 | 1 | — |
| scafra-macros | 5 | 0 | 1 | scafra-build, scafra-foundation |
| scafra-scheduler | 5 | 0 | 3 | — |
| scafra-security | 8 | 0 | 1 | — |
| scafra-web | 12 | 0 | 3 | scafra-actuator, scafra-core, scafra-foundation, scafra-security |

## scafra

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | jsonwebtoken | ^9 | all | — | false / true |
| dev | scafra-bootui | ^0.1.0 | all | — | false / true |
| dev | serde | ^1 | all | derive | false / true |
| dev | serde_json | ^1 | all | — | false / true |
| dev | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time, test-util | false / true |
| dev | tower | ^0.5 | all | util | false / true |
| dev | tracing-subscriber | ^0.3 | all | env-filter, fmt | false / true |
| normal | axum | ^0.8 | all | — | false / true |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | scafra-actuator | ^0.1.0 | all | — | false / true |
| normal | scafra-config | ^0.1.0 | all | — | false / true |
| normal | scafra-core | ^0.1.0 | all | — | false / true |
| normal | scafra-foundation | ^0.1.0 | all | — | false / true |
| normal | scafra-macros | ^0.1.0 | all | — | false / true |
| normal | scafra-scheduler | ^0.1.0 | all | — | false / true |
| normal | scafra-security | ^0.1.0 | all | — | false / true |
| normal | scafra-web | ^0.1.0 | all | — | false / true |
| normal | thiserror | ^2 | all | — | false / true |
| normal | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time | false / true |
| normal | tracing | ^0.1 | all | — | false / true |

## scafra-actuator

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | serde_json | ^1 | all | — | false / true |
| dev | tower | ^0.5 | all | util | false / true |
| normal | axum | ^0.8 | all | — | false / true |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | serde | ^1 | all | derive | false / true |
| normal | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time | false / true |
| normal | tracing | ^0.1 | all | — | false / true |

## scafra-bootui

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | serde_json | ^1 | all | — | false / true |
| dev | serde_yaml | ^0.9 | all | — | false / true |
| dev | tower | ^0.5 | all | util | false / true |
| normal | axum | ^0.8 | all | — | false / true |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | scafra-config | ^0.1.0 | all | — | false / true |
| normal | scafra-core | ^0.1.0 | all | — | false / true |
| normal | scafra-security | ^0.1.0 | all | — | false / true |
| normal | serde | ^1 | all | derive | false / true |
| normal | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time | false / true |
| normal | tracing | ^0.1 | all | — | false / true |

## scafra-build

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| normal | proc-macro2 | ^1 | all | span-locations | false / true |
| normal | scafra-core | ^0.1.0 | all | — | false / true |
| normal | scafra-foundation | ^0.1.0 | all | — | false / true |
| normal | syn | ^2 | all | full, extra-traits | false / true |

## scafra-cli

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | serde_json | ^1 | all | — | false / true |
| normal | anyhow | ^1 | all | — | false / true |
| normal | clap | ^4.5 | all | derive | false / true |
| normal | rustix | ^1.1 | cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple", target_os = "redox")) | fs | false / true |
| normal | scafra-foundation | ^0.1.0 | all | — | false / true |
| normal | windows-sys | ^0.60 | cfg(windows) | Win32_Storage_FileSystem | false / true |

## scafra-config

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | scafra-actuator | ^0.1.0 | all | — | false / true |
| normal | scafra-foundation | ^0.1.0 | all | — | false / true |
| normal | scafra-scheduler | ^0.1.0 | all | — | false / true |
| normal | scafra-security | ^0.1.0 | all | — | false / true |
| normal | serde | ^1 | all | derive | false / true |
| normal | serde_yaml | ^0.9 | all | — | false / true |
| normal | thiserror | ^2 | all | — | false / true |

## scafra-core

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | scafra-foundation | ^0.1.0 | all | — | false / true |
| normal | thiserror | ^2 | all | — | false / true |

## scafra-foundation

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | serde_json | ^1 | all | — | false / true |
| normal | serde | ^1 | all | derive | false / true |
| normal | thiserror | ^2 | all | — | false / true |
| normal | tracing | ^0.1 | all | — | false / true |
| normal | tracing-subscriber | ^0.3 | all | env-filter, fmt | false / true |

## scafra-macros

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | trybuild | ^1 | all | — | false / true |
| normal | proc-macro2 | ^1 | all | — | false / true |
| normal | quote | ^1 | all | — | false / true |
| normal | scafra-build | ^0.1.0 | all | — | false / true |
| normal | scafra-foundation | ^0.1.0 | all | — | false / true |
| normal | syn | ^2 | all | full, extra-traits | false / true |

## scafra-scheduler

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | serde_yaml | ^0.9 | all | — | false / true |
| dev | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time, test-util | false / true |
| dev | tracing-subscriber | ^0.3 | all | env-filter, fmt | false / true |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | serde | ^1 | all | derive | false / true |
| normal | thiserror | ^2 | all | — | false / true |
| normal | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time | false / true |
| normal | tracing | ^0.1 | all | — | false / true |

## scafra-security

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | tower | ^0.5 | all | util | false / true |
| normal | axum | ^0.8 | all | — | false / true |
| normal | base64 | ^0.22 | all | — | false / true |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | jsonwebtoken | ^9 | all | — | false / true |
| normal | reqwest | ^0.12 | all | json, rustls-tls | false / false |
| normal | serde | ^1 | all | derive | false / true |
| normal | serde_json | ^1 | all | — | false / true |
| normal | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time | false / true |

## scafra-web

| Kind | Dependency | Requirement | Target | Dependency features | Optional / defaults |
| --- | --- | --- | --- | --- | --- |
| dev | jsonwebtoken | ^9 | all | — | false / true |
| dev | serde | ^1 | all | derive | false / true |
| dev | tracing-subscriber | ^0.3 | all | env-filter, fmt | false / true |
| normal | axum | ^0.8 | all | — | false / true |
| normal | inventory | ^0.3 | all | — | false / true |
| normal | scafra-actuator | ^0.1.0 | all | — | false / true |
| normal | scafra-core | ^0.1.0 | all | — | false / true |
| normal | scafra-foundation | ^0.1.0 | all | — | false / true |
| normal | scafra-security | ^0.1.0 | all | — | false / true |
| normal | serde | ^1 | all | derive | false / true |
| normal | thiserror | ^2 | all | — | false / true |
| normal | tokio | ^1 | all | io-util, macros, net, rt-multi-thread, signal, sync, time | false / true |
| normal | tower | ^0.5 | all | util | false / true |
| normal | tower-http | ^0.6 | all | timeout, trace | false / true |
| normal | tracing | ^0.1 | all | — | false / true |

Counts are dependency declarations, so Tokio normal and dev entries count in their respective categories. CLI has mutually exclusive target dependencies; its declaration count is not its Linux direct build count. Workspace dependency defaults (for example Tokio networking and tracing formatting) are inherited by each referencing manifest.

See [source audit](dependency-audit.md) for responsibilities, independent-build evidence, removable edges, and cycle analysis. Regenerate machine-readable inventory and selected-target normal/feature trees with `python3 scripts/arch103-baseline.py /var/tmp/arch103-inventory-NEW`.
