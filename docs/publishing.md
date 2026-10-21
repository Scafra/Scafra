# Publishing releases

Pull requests and branch pushes run formatting, workspace checks, Clippy with
warnings denied, and tests. Stable releases publish when a matching tag such
as `v0.1.0` is pushed. Snapshots publish only when manually started from GitHub
Actions.

## One-time setup

1. Create a crates.io API token with permission to publish the Scafra packages.
2. In the GitHub repository settings, add it as the Actions secret
   `CRATES_IO_TOKEN`.

The first successful release claims the package names on crates.io. The names
were checked during setup, but crates.io package names can be claimed by
someone else before the first release; the publish job will fail clearly if
that happens.

Both publishing paths run formatting, workspace, Clippy, and test gates, then
check package manifests and file lists before using the same crates.io token
to publish in dependency order. Each publish verifies the crate after its
dependencies are available. After each upload the workflow waits for crates.io's
index to expose that exact version before moving on to dependents. The stable
path verifies its tag against
`[workspace.package].version`; the snapshot path applies its selected
prerelease version only inside the runner, leaving the branch unchanged.

## Snapshot releases

In GitHub, open **Actions → Publish to crates.io → Run workflow** and enter a
new prerelease version, for example `0.1.0-snapshot.1` or `0.1.0-alpha.1`.
Use a different version for every snapshot because published crate versions
cannot be reused. Users can try a snapshot with an explicit prerelease
requirement:

```toml
[dependencies]
scafra = { version = "=0.1.0-snapshot.1", default-features = false, features = ["web"] }
```

Snapshots and stable releases share package names, but prerelease versions do
not satisfy ordinary stable version requirements.

## Release steps

1. Set the shared version in the root `Cargo.toml` and commit the release.
2. Push the release commit to the repository's normal release branch.
3. Create and push the matching tag:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

The publish order is defined in `.github/workflows/publish.yml`. Internal
dependencies are published before packages that depend on them. If a run
stops partway through, rerun the failed workflow: package versions already
visible in the crates.io index are skipped, and publication continues with the
first missing package.

## Package names

Scafra publishes its application-facing facade as `scafra` and its companion
packages under the `scafra-*` prefix. Applications can depend on the facade:

```toml
[dependencies]
scafra = { version = "0.1", default-features = false, features = ["web"] }
```

Then import the framework with `use scafra::prelude::*`. The CLI package is
`scafra-cli` and installs the `scafra` executable.

## Modular release verification

The facade has empty default features. Select `web` for a minimal HTTP app,
`security` for Basic/static bearer enforcement, `jwt` for local JWT validation,
and `jwks` for remote JWKS/discovery. `actuator`, `scheduler`, and `bootui` are
explicit additions. `standard` restores the former runtime capability set
without BootUI. Cargo features are additive across consumers, so an application
must inspect its resolved normal dependency closure as well as its manifest.
Generic configuration and core lifecycle remain independently usable without
HTTP. A supervised worker entry point remains outside this modularity change.

Before packaging, validate both workflow lists against Cargo's actual internal
normal and build dependencies, including optional integrations:

```bash
python3 scripts/arch103-publish-order.py --check
```

In particular, `scafra-build` must precede `scafra-macros`. The checker fails for
missing packages, duplicates, differing packaging/publishing arrays, or an
out-of-order dependency. Development-only dependencies do not constrain
publication. The workflow runs this check before package enumeration.

CI retains workspace all-feature checks and separately runs isolated external
consumers for minimal/default facade, direct config/core, web, secure-web, JWT,
JWKS, actuator, scheduler, standard, and all features. The consumer matrix uses the declared MSRV (1.88.0), plus a
stable minimal build. Each invocation uses one consumer selection to avoid
workspace feature unification masking missing APIs or unwanted dependencies.
The consumer script checks normal dependency isolation and exercises startup
and shutdown for applicable runtime fixtures. Consumer jobs fetch locked registry
dependencies before offline builds. The minimal jobs additionally run the
negative feature/configuration regression script.

For local package review, `cargo package --package NAME --locked --allow-dirty
--list` checks file selection. Full `cargo package` also resolves registry
versions of inter-crate dependencies and compiles the extracted package; it can
fail before the matching dependencies have been published. Path-based consumer
checks cannot prove registry resolution. The release job publishes in dependency
order and verifies each dependent only after its exact dependency versions are
visible in the registry. Never use `--no-verify` to claim verified packaging.
No local validation command above publishes or triggers a release.

CLI-generated HTTP applications explicitly select `web` and disable defaults.
`scafra new my-api --kind api --features security,actuator` selects additional
capabilities without a project wizard. Security/JWT profiles enable enforcement
but require credentials via configuration/environment before startup; generated
files contain no placeholder secret or public fallback. See the generated README
for the required environment keys. Existing applications migrating from the
previous implicit module set can initially select `standard`.

Release candidate review should apply the API stability (#16), MSRV (#33),
release verification (#42), and 1.0.0 checklist (#39) gates to these same packaged
feature selections. A passing workspace build alone does not complete them.

Local verification during #103 implementation packaged all twelve file lists
successfully and fully verified the extracted `scafra-foundation` package.
Full `scafra-cli` packaging failed both online and offline because crates.io
contained `scafra-foundation 0.1.0-snapshot.1`, but not the required stable
`^0.1.0`. This is an unresolved registry prerequisite for prepublication local
verification, not evidence that dependent release packages have passed. No
crates were published. The release workflow supplies that prerequisite by
publishing and waiting for the foundation version before dependents.

A stronger check can stage the normalized `.crate` archives into a disposable
Cargo directory registry source before any release exists:

```bash
python3 scripts/arch103-package-check.py /var/tmp/scafra-package-review
```

Use a new output directory outside the checkout. `--offline` requires every
locked external dependency to be cached. The script vendors external packages,
fully verifies each extracted package in publish order, then stages it with
checksums so later packages resolve its registry version. Finally it checks
independent `web` and `standard` macro consumers using version-only dependencies
and no workspace patches. Logs, archives, and `results.json` remain under the
output directory; none belong in Git. `--reuse` is reserved for retrying an
output created by this script. This proves local directory registry-source
resolution and package compilation, not actual crates.io index or upload
acceptance. It neither publishes nor requires publishing credentials.

The staged-source procedure passed for all twelve current `0.1.0` packages and
both registry-source macro consumers during implementation. The direct crates.io
resolution limitation above still applies until stable dependencies are
published. Local staged success must not be reported as a crates.io upload or
crates.io index-resolution test.
