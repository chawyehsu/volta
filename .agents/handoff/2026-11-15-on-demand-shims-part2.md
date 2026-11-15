# Handoff: on-demand shims, Phase 2 (v3)

Phase 1 (shipped in v2.x) makes `volta install`/`volta pin` create shims on demand in `$VOLTA_HOME/bin` without removing anything. See `.agents/adr/0013-on-demand-shims.md` for the rationale.

## Goal

Make a fresh Volta install consist of only `volta`, `volta-shim`, and `volta-migrate`, with all shims created on demand in `$VOLTA_HOME/bin` on every platform.

## Work items

1. **Remove MSI shim copies** — `wix/main.wxs`: drop the `nodeBinary`/`npmBinary`/`npxBinary`/`pnpmBinary`/`yarnBinary`/`yarnPkgBinary` components and the `wix/shim.cmd` wrappers. Keep the `volta.exe` PATH entry and the `volta setup` custom action.
2. **Remove pre-created defaults** — drop the hard-coded `["node","npm","npx","pnpm","yarn","yarnpkg"]` list from `crates/volta-core/src/shim.rs::get_shim_list_deduped`. `volta setup`/migration should only repair existing shims.
3. **Unify Windows shims** — decide whether runtime tool shims stay `.exe` copies or whether the `.cmd` script model is removed entirely. If `.exe` copies win, teach `regenerate_shims_for_dir`, `delete`, `entry_to_shim_name`, and `check_shim_reachable` about them; `check_shim_reachable` should stop accepting `volta_install().root()`.
4. **Migration/backfill** — existing users must not lose shims. Likely a layout bump (V5) whose upgrade path backfills the default shims on Windows and removes stale `Program Files` copies, while the fresh-install path creates none. Distinguish fresh vs. upgrade in `detect_and_migrate` (route `Empty` directly to the new layout).
5. **Unix installer scripts** — `dev/unix/install.sh.in` and `dev/unix/volta-install-legacy.sh` still pre-create symlinks. Either update them or leave them until the planned installer rewrite.
6. **Tests/docs** — update `tests/acceptance/migrations.rs` (fresh install no longer creates default shims), testkit `ShimBuilder`/`smoke.rs`, and the glossary.

## Notes

- `volta uninstall` must not remove runtime tool shims (ADR 0011).
- On Windows, removing files from `Program Files` requires the same privileges as `volta self-update`; treat permission failures as best-effort with a debug log.
