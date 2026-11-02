Please see https://docs.volta.sh/contributing/

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
