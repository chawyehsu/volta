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
