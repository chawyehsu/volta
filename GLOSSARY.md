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
The Volta command that downloads a newer stable release for the current target triplet, verifies it, and replaces the installed Volta binaries.
_Avoid_: Upgrade, auto-update

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
