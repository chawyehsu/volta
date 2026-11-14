Please see https://docs.volta.sh/contributing/

# Build cache

Local and CI builds share compiler outputs through [kache](https://github.com/kunobi-ninja/kache), configured in `pixi.toml` as the `RUSTC_WRAPPER` for `pixi build` and for the `test` and `coverage` environments.

The cache only works when `scripts/env.ps1` has been sourced. It points the kache store and runtime directory at the repository root, keeps `CARGO_HOME` on the host, and removes any `SCCACHE*` variables so the pixi-build backend does not switch the wrapper back to sccache:

```pwsh
. ./scripts/env.ps1
pixi run build
```

`build.ps1` sources the same script, so `./build.ps1 -Platform linux` needs no manual setup. Show cache activity with `pixi run -e test cache-stats`.

## macOS build noise

Native Apple Silicon builds print two warnings that do not affect the artifact:

- `--sign is required` comes from kache's macOS loadability check. kache runs `codesign --verify`, but the build sandbox's `codesign` is the `sigtool` shim, which only implements `--sign`. kache then re-signs with `codesign --sign - --force`, which succeeds, and the binaries are validly ad-hoc signed.
- `warning: no debug symbols in executable` comes from kache's `dsymutil` run, which caches a `.dSYM` for executables. `[profile.release]` sets `strip = "debuginfo"`, so there is nothing to bundle; kache skips the bundle and caches the binary.

Both steps are best-effort. CI's Linux cross-build does not produce them.

# Release smoke checklist

macOS (x86-64 and Apple Silicon) release artifacts are cross-compiled from Linux, and macOS-specific logic is covered by platform-parameterized tests that run on Linux. Because CI no longer executes binaries on macOS, a manual macOS smoke test is required before each release.

## test on macOS

CI no longer executes Volta on macOS. Before publishing a release, run this checklist on a real macOS machine for each affected architecture (`aarch64-apple-darwin`, `x86_64-apple-darwin`). Use an isolated `HOME` and `VOLTA_HOME` so the check does not modify your normal setup.

- [ ] Download `volta-<triple>.tar.gz` and its `.sha256` sidecar from the draft release, then verify the checksum.
- [ ] Extract the archive and confirm `volta`, `volta-shim`, and `volta-migrate` are present and executable.
- [ ] Run `./volta --version` and confirm it reports the release version.
- [ ] Run `./volta install node@lts` and confirm it succeeds.
- [ ] Run `./volta run node --version` and confirm it prints the installed Node version.
- [ ] Create a temporary project, run `./volta pin node@lts`, and confirm a shim in `$VOLTA_HOME/bin` runs Node.
- [ ] Run `./volta setup` with the isolated `HOME` and confirm it updates the expected shell profile.
