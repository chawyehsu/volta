# Shims are created on demand, in two phases

Volta's release archive contains only `volta`, `volta-shim`, and `volta-migrate`, but the Windows MSI additionally pre-creates copies of `volta-shim` (`node.exe`, `npm.exe`, …) in `Program Files\Volta`, while Unix installers and `volta setup` pre-create `node`/`npm`/`npx`/`pnpm`/`yarn`/`yarnpkg` symlinks. The two platforms therefore disagree about where shims live and how many exist, and Windows zip installs get no default shims at all. Community reports (volta-cli/volta#1473, volta-cli/volta#1554, volta-cli/volta#2021) trace back to this inconsistency.

We will move to a model where a fresh install contains only the three core files and shims are created on demand when a tool becomes in use. Because removing the pre-created shims outright would strand existing users, the change ships in two phases:

- **Phase 1 (this change, non-breaking):** `volta install <tool>` and `volta pin <tool>` ensure the tool's shims exist in the Volta home bin directory. On Unix they are symlinks to `volta-shim`; on Windows they are `.exe` copies of `volta-shim`, refreshed when the copy's size or hash differs from the current binary. The MSI, the pre-created default shims, `volta setup`, migration, and uninstall behavior are unchanged. A shim failure warns and does not fail the install.
- **Phase 2 (deferred to v3):** remove the MSI shim copies and the hard-coded default shims, unify Windows on `$VOLTA_HOME/bin`, and migrate existing installs (likely via a layout bump so the cleanup/backfill runs exactly once). Fresh installs in Phase 2 will have no shims until a tool is installed.

## Considered Options

- **Ship the whole change at once** — rejected: it is breaking for existing Windows MSI users, who would lose their default shims on upgrade, forcing a v3 release. Phase 1 captures most of the benefit (zip installs get shims, installs become self-consistent) without that cost.
- **A V5 layout bump in Phase 1** — rejected: a layout bump is itself a breaking change to the on-disk state. Phase 1 stays strictly additive.
- **Windows `.cmd` script shims for runtime tools** — rejected: the existing `.cmd` shims are tied to the `.cmd`-oriented regeneration/deletion machinery. `.exe` copies are simpler, need no shell wrapper, and can reuse the self-update copy primitive.

## Consequences

- Phase 1 leaves Windows MSI installs with two shim sources (`Program Files` `.exe` copies and `$VOLTA_HOME/bin` `.exe` copies). The former wins on `PATH`; both dispatch through `volta run`, so behavior is unchanged.
- `check_shim_reachable` accepts either location on Windows during Phase 1 to avoid false "shadowed" warnings.
- Phase 1's `.exe` shims are a separate lifecycle from `.cmd` shims: `regenerate_shims_for_dir` and `delete` do not touch them until Phase 2 unifies the model. `volta uninstall` removes them through a dedicated tool-shim path instead.
- Tool shims are removed by `volta uninstall` only once their tool has no cached versions left and no remaining reference (a cached version, the global default, a project pin, or a global-package platform) still needs the shim (see ADR 0011).

## Revision

- 2026-11-15: Tool shims are now removed by `volta uninstall` when their tool has no cached versions left and no remaining reference still needs them.
