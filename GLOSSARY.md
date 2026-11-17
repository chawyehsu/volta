# Volta

Volta is a JavaScript tool manager that installs and runs Node.js, npm, pnpm, and Yarn.

## Language

**Host platform**:
The operating system and architecture a Volta binary was compiled for; it determines which Node distribution is downloaded.
_Avoid_: platform, OS, target

**Node distro**:
The Node.js archive naming for a host platform, including the optional older-version fallback used by Apple Silicon and Windows ARM.
_Avoid_: Node build, Node tarball

**Runner label**:
The GitHub Actions `runs-on` image name, such as `ubuntu-latest` or `windows-2022`.
_Avoid_: platform

**Pixi platform**:
A conda target name such as `linux-64` or `osx-arm64`.
_Avoid_: platform

**Target triple**:
A Rust target identifier such as `aarch64-apple-darwin`, used in release asset names.
_Avoid_: platform

## Project resolution vocabulary

**Project root**:
The nearest ancestor directory containing a `package.json`, excluding directories inside `node_modules`.
_Avoid_: workspace root, repo root

**Node version source**:
The input that determines a project's Node version: either `volta.node` in `package.json` or a `.node-version` file.
_Avoid_: version pin, version config

**Version file**:
A `.node-version` file that requests a Node version for a project. Volta reads it but never creates or modifies it.
_Avoid_: pin file, version pin

**Partial version**:
A version request with fewer than three numeric components, such as `20` or `20.5`, resolved to the highest matching Node release.
_Avoid_: loose version, range

## Toolchain vocabulary

**Image**:
The unpacked, on-disk copy of one version of a runtime tool in the Volta home directory.
_Avoid_: installed tool, unpacked runtime

**Inventory**:
The cache of downloaded tool-version archives in the Volta home directory.
_Avoid_: download cache, archive store

**Cached version**:
A fetched version of a runtime tool kept in the Volta home directory, consisting of an unpacked image and its archive in the inventory.
_Avoid_: installed version, downloaded version

**In-use version**:
A cached version that is the user's default, pinned in the current project, or recorded as the install-time platform of an installed global package.
_Avoid_: active version, live version

**Uninstall**:
The Volta command that removes cached versions of a runtime tool from the local machine without changing any configuration, and removes tool shims that no remaining reference needs; removed versions are re-downloaded automatically when next needed.
_Avoid_: clean, prune, remove

**Launcher**:
The script Volta writes into a tool image's bin directory, invoking the tool's JavaScript entrypoint with `node` from the execution environment.
_Avoid_: wrapper script, bootstrap

**Shim**:
The executable stub in the Volta bin directory that dispatches a command to the tool version selected by the active platform.
_Avoid_: proxy, wrapper

**Tool shim**:
A shim created when a runtime tool is installed or pinned, so that the tool's commands are available through Volta; it is removed when the tool's last cached version is uninstalled and no default, project pin, or global package still needs it.
_Avoid_: on-demand shim, install shim

**Default shim**:
A shim pre-created for a well-known command before any tool is installed, so that Volta can delegate the command to the system when no Volta-managed version is active.
_Avoid_: built-in shim, bundled shim

## Build cache vocabulary

**Build cache**:
The content-addressed store of compiler outputs that lets a rebuild skip recompiling unchanged crates.
_Avoid_: compilation cache, sccache

**kache store**:
The persistent on-disk portion of the build cache, holding cached compiler outputs and their index.
_Avoid_: cache directory, cache folder

**kache runtime directory**:
The ephemeral directory holding kache's sockets, locks, logs, and session state; it is not persisted between CI jobs.
_Avoid_: cache directory, temp cache

## Release vocabulary

**Release**:
A stable, versioned Volta publication on GitHub, identified by its release tag.
_Avoid_: Build, artifact

**Release asset**:
A file attached to a GitHub Release for a particular platform target.
_Avoid_: Artifact when referring to a published file

**Target triplet**:
The canonical Rust target identifier describing the operating system, architecture, and toolchain of a Volta build.
_Avoid_: Platform name, architecture name

**Archive**:
A compressed release asset containing the Volta executable, shim, and migration executable for one target triplet.
_Avoid_: Package when referring to a self-update asset

**Checksum sidecar**:
A GNU-compatible SHA-256 text file paired with one archive and identifying that archive by filename.
_Avoid_: Checksum file when the archive relationship matters

**Self-update**:
The Volta command that installs a selected release of Volta for the current target triplet — the latest stable release by default, or an explicitly requested version — after verifying it and replacing the installed binaries.
_Avoid_: Upgrade, auto-update

**Requested version**:
The optional version argument to `self-update`; it selects that exact release instead of the latest stable release.
_Avoid_: Target version, pinned version

## Self-update testing vocabulary

**Fixture release**:
A synthetic GitHub Release used only by tests, describing a version and the fixture archives and checksum sidecars for the supported target triplets.
_Avoid_: Mock release, test release

**Fixture archive**:
A generated release archive used only by tests to exercise self-update for one target triplet.
_Avoid_: Test archive, fake archive

**Fixture install directory**:
A temporary directory containing copies of the Volta binaries that a self-update test is allowed to replace.
_Avoid_: Sandbox install directory

## Test vocabulary

**Sandbox**:
The isolated per-test environment, comprising temporary home and project directories, a private `PATH` of Volta shims, and a mock download server, in which tests run the compiled Volta binaries.
_Avoid_: TestWorkspace, TempProject
