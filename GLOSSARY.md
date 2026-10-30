# Volta

This context defines the language used for the Volta project.

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
