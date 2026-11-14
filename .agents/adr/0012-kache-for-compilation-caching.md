# Compilation caching moves from sccache to kache

Volta's builds cached rustc output with sccache, using its GitHub Actions cache service v2 backend in CI. We replace it with [kache](https://github.com/kunobi-ninja/kache) 1.2.2 from conda-forge, pinned in `pixi.toml`. kache is content-addressed, works as a plain `RUSTC_WRAPPER`, and keeps its cache in a directory, so the same integration serves both the isolated `pixi build` sandbox and the direct `cargo test`/`cargo llvm-cov` jobs.

kache 1.x has no GitHub Actions cache-service backend, so CI persists the store with `actions/cache` instead of kache talking to the cache service. The store lives at `./.kache`, outside `.pixi`, so `pixi build --clean` cannot wipe it. `KACHE_CACHE_DIR`, `KACHE_RUNTIME_DIR`, and `CARGO_HOME` reach the build sandbox through `[package.build] secrets`.

One constraint drives the setup: `pixi-build-rust` force-exports `RUSTC_WRAPPER="sccache"` whenever any `SCCACHE*` variable is visible, which would silently disable kache. `scripts/env.ps1` and the workflows remove them. Local builds source `scripts/env.ps1`; `build.ps1` sources it too.

## Considered Options

- **`kunobi-ninja/kache-action`** — provides GitHub cache persistence and PR summaries, but installs its own kache binary outside the pixi build sandbox. The sandbox only sees the conda-forge kache build-dependency, so the action cannot drive `pixi build`; using it would mean two binaries and two stores.
- **S3/GCS/OCI remote** — the only native remote transports in kache 1.x. Rejected: extra infrastructure and credentials for a build cache whose main consumer is this repository's own CI.
- **Keep sccache for `test`/`coverage`, kache for `pixi build`** — rejected: two caches and two configuration surfaces, and the task was to migrate off sccache.

## Consequences

- Cross-job cache reuse is coarser than sccache's live GitHub cache service: each job restores and saves a directory snapshot keyed on OS, architecture, job name, and `Cargo.lock`, within GitHub's 10 GB per-repository limit.
- kache's store and runtime directories must be passed into the pixi build sandbox explicitly, because its `HOME` and working directory are throwaway build paths.
- Coverage runs stay cached; kache keys instrumented units separately, so they cannot be restored into non-instrumented builds.
- If a future kache release ships a native GitHub Actions backend, the `actions/cache` steps can be replaced without changing the build configuration.
