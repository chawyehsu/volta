# `volta uninstall` removes cached tool versions locally

Volta's fetch-on-demand model means a runtime tool (Node, npm, pnpm, Yarn) is downloaded the first time it is needed and kept in the Volta home directory afterwards. `volta uninstall` existed only as a stub for global packages: uninstalling runtimes was unsupported (volta-cli/volta#327), and the only way to reclaim disk space was deleting directories by hand. Draft RFC #53 and PoCs #1658/#1882 explored the space without settling the semantics.

We will treat `volta uninstall` as a purely local cache-removal operation with this contract:

- It accepts runtime tools and multiple tool specs per invocation, mirroring `volta install` argument parsing and validation. Global package behavior is unchanged, and `package@version` remains unsupported.
- An exact version (`node@18.20.2`) removes that one cached version. A version range (`node@18`) removes every locally cached version matching the range. A bare tool name (`volta uninstall node`) removes all cached versions. Matching is performed against the locally unpacked images only; nothing is resolved against a remote index.
- Tag-based specs (`node@latest`, `node@lts`, custom tags) are rejected with an error: resolving them requires the remote index, which uninstall never consults.
- Removal deletes the unpacked image, the cached inventory archive, and, for Node, the bundled npm metadata file. Removing only the image would not reclaim the disk space that motivates the command.
- Versions that are in use — set as the global default, pinned in the current project, or recorded as the install-time platform of an installed global package — are skipped by range and bare-name matches, with a warning naming the reasons. When targeted by an exact version, they are removed with a warning instead. In both cases the configuration (`platform.json`, the project's `package.json`) is left untouched: uninstall never unpins.
- The command stays non-interactive: no confirmation prompt and no force flag. A spec that matches nothing warns and exits 0; a hard filesystem error aborts remaining tools.
- A `VoltaLock` is held during removal to prevent concurrent changes.

## Considered Options

- **Resolve ranges against the remote index like `install`** — rejected: users expect `volta uninstall node@10` to remove their local 10.x versions, not whichever single 10.x release is currently newest; resolution would also make uninstall unusable offline.
- **Interactive confirmation for wide-ranging removals (floated in RFC #53)** — rejected: Volta has no interactive prompts anywhere, and prompts hang scripts and CI. The in-use protection rules plus per-version output carry the safety instead.
- **Separate `clean`/`prune`/`remove` verb** — rejected: those verbs were tied to the unmerged RFC #52 (deprecating `install`); `volta uninstall` already exists as a stub and matches the long-standing issue.
- **Delete only the unpacked image, keep the cached archive** — rejected: reclaiming disk space is the primary motivation, and the archive is typically the larger half; keeping it only speeds up a re-fetch.
- **Ignore package-recorded platforms when deciding "in use"** — rejected: global package binaries run against their install-time platform, so removing such a version silently forces a re-download on the package's next run. Counting them avoids repeat downloads at the cost of keeping those versions protected.

## Consequences

- A removed version that is still referenced (default, project pin, package platform) is transparently re-downloaded the next time it is needed; removing even the default node does not break the environment, it only costs a download.
- Package-manager update commands for runtimes (`npm update -g node`, `yarn global upgrade node`, `pnpm update -g node`) are rejected with an error rather than routed to the uninstall logic, which would otherwise delete the cached versions instead of updating anything.
- A global package's recorded platform is a snapshot from its install time; it keeps that Node version in use until the package is reinstalled or upgraded, which rewrites the config.
- Versions whose image is missing (for example, an interrupted fetch) are not matched by uninstall, even if their archive is still cached.
- A new `UninstallTagUnsupported` error kind reports rejected tags with exit code `InvalidArguments`.
- If RFC #52 later replaces `install` with a different verb, the surface here can be renamed without changing these semantics.
