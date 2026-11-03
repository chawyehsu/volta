use std::collections::HashSet;
use std::fmt::{self, Display};

use super::{
    FetchStatus, Tool, check_fetched, check_shim_reachable, debug_already_fetched, info_fetched,
    info_installed, info_pinned, info_project_version,
};
use crate::error::{ErrorKind, Fallible};
use crate::inventory::node_available;
use crate::session::Session;
use crate::style::{note_prefix, tool_version};
use crate::sync::VoltaLock;
use cfg_if::cfg_if;
use log::info;
use node_semver::Version;

mod fetch;
mod metadata;
mod resolve;

pub use fetch::load_default_npm_version;
pub use resolve::resolve;

/// A platform for which Volta can select a Node distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPlatform {
    WindowsX86,
    WindowsX64,
    WindowsArm64,
    MacosX64,
    MacosArm64,
    LinuxX64,
    LinuxArm64,
    LinuxArm,
}

impl HostPlatform {
    /// Every platform for which Volta can select a Node distribution.
    pub const ALL: [HostPlatform; 8] = [
        HostPlatform::WindowsX86,
        HostPlatform::WindowsX64,
        HostPlatform::WindowsArm64,
        HostPlatform::MacosX64,
        HostPlatform::MacosArm64,
        HostPlatform::LinuxX64,
        HostPlatform::LinuxArm64,
        HostPlatform::LinuxArm,
    ];

    /// The platform this binary was compiled for.
    pub const fn current() -> Self {
        CURRENT
    }

    /// Whether this platform is macOS.
    pub const fn is_macos(self) -> bool {
        matches!(self, Self::MacosX64 | Self::MacosArm64)
    }

    /// The Node distribution used by this platform.
    pub const fn node_distro(self) -> NodeDistro {
        match self {
            Self::WindowsX86 => NodeDistro {
                os: "win",
                arch: "x86",
                extension: "zip",
                identifier: "win-x86-zip",
                fallback: None,
            },
            Self::WindowsX64 => NodeDistro {
                os: "win",
                arch: "x64",
                extension: "zip",
                identifier: "win-x64-zip",
                fallback: None,
            },
            Self::WindowsArm64 => NodeDistro {
                os: "win",
                arch: "arm64",
                extension: "zip",
                identifier: "win-arm64-zip",
                // Node support for pre-built ARM64 binaries on Windows was added in major version 20.
                // For versions prior to that, we fall back on the x64 binaries via emulator.
                fallback: Some(NodeDistroFallback {
                    arch: "x64",
                    identifier: "win-x64-zip",
                    min_major: 20,
                }),
            },
            Self::MacosX64 => NodeDistro {
                os: "darwin",
                arch: "x64",
                extension: "tar.gz",
                identifier: "osx-x64-tar",
                fallback: None,
            },
            Self::MacosArm64 => NodeDistro {
                os: "darwin",
                arch: "arm64",
                extension: "tar.gz",
                identifier: "osx-arm64-tar",
                // Node support for pre-built Apple Silicon binaries was added in major version 16.
                // For versions prior to that, we fall back on the x64 binaries via Rosetta 2.
                fallback: Some(NodeDistroFallback {
                    arch: "x64",
                    identifier: "osx-x64-tar",
                    min_major: 16,
                }),
            },
            Self::LinuxX64 => NodeDistro {
                os: "linux",
                arch: "x64",
                extension: "tar.gz",
                identifier: "linux-x64",
                fallback: None,
            },
            Self::LinuxArm64 => NodeDistro {
                os: "linux",
                arch: "arm64",
                extension: "tar.gz",
                identifier: "linux-arm64",
                fallback: None,
            },
            Self::LinuxArm => NodeDistro {
                os: "linux",
                arch: "armv7l",
                extension: "tar.gz",
                identifier: "linux-armv7l",
                fallback: None,
            },
        }
    }
}

cfg_if! {
    if #[cfg(all(target_os = "windows", target_arch = "x86"))] {
        const CURRENT: HostPlatform = HostPlatform::WindowsX86;
    } else if #[cfg(all(target_os = "windows", target_arch = "x86_64"))] {
        const CURRENT: HostPlatform = HostPlatform::WindowsX64;
    } else if #[cfg(all(target_os = "windows", target_arch = "aarch64"))] {
        const CURRENT: HostPlatform = HostPlatform::WindowsArm64;
    } else if #[cfg(all(target_os = "macos", target_arch = "x86_64"))] {
        const CURRENT: HostPlatform = HostPlatform::MacosX64;
    } else if #[cfg(all(target_os = "macos", target_arch = "aarch64"))] {
        const CURRENT: HostPlatform = HostPlatform::MacosArm64;
    } else if #[cfg(all(target_os = "linux", target_arch = "x86_64"))] {
        const CURRENT: HostPlatform = HostPlatform::LinuxX64;
    } else if #[cfg(all(target_os = "linux", target_arch = "aarch64"))] {
        const CURRENT: HostPlatform = HostPlatform::LinuxArm64;
    } else if #[cfg(all(target_os = "linux", target_arch = "arm"))] {
        const CURRENT: HostPlatform = HostPlatform::LinuxArm;
    } else {
        compile_error!("Unsupported operating system + architecture combination");
    }
}

/// The Node distribution naming used on a particular platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeDistro {
    /// The OS component of a Node distro filename
    pub os: &'static str,
    /// The architecture component of a Node distro filename
    pub arch: &'static str,
    /// The extension for Node distro files
    pub extension: &'static str,
    /// The file identifier in the Node index `files` array
    pub identifier: &'static str,
    /// The fallback Node distribution for this platform, if any.
    pub fallback: Option<NodeDistroFallback>,
}

/// The older Node distribution a platform can fall back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeDistroFallback {
    /// The architecture component of the fallback Node distro filename
    pub arch: &'static str,
    /// The file identifier in the Node index `files` array for the fallback
    pub identifier: &'static str,
    /// The minimum major version of Node that requires this fallback
    pub min_major: u64,
}

impl NodeDistro {
    /// The archive basename for `version`, honoring the platform fallback.
    pub fn archive_basename(&self, version: &Version) -> String {
        let arch = match self.fallback {
            Some(fallback) if version.major < fallback.min_major => fallback.arch,
            _ => self.arch,
        };
        format!("node-v{}-{}-{}", version, self.os, arch)
    }

    /// The archive filename for `version`, honoring the platform fallback.
    pub fn archive_filename(&self, version: &Version) -> String {
        format!("{}.{}", self.archive_basename(version), self.extension)
    }

    /// Whether the Node index advertises a build for this platform.
    pub fn supports(&self, files: &HashSet<String>) -> bool {
        files.contains(self.identifier)
            || self
                .fallback
                .is_some_and(|fallback| files.contains(fallback.identifier))
    }
}

/// The OS component of a Node distro filename
pub const NODE_DISTRO_OS: &str = HostPlatform::current().node_distro().os;
/// The architecture component of a Node distro filename
pub const NODE_DISTRO_ARCH: &str = HostPlatform::current().node_distro().arch;
/// The extension for Node distro files
pub const NODE_DISTRO_EXTENSION: &str = HostPlatform::current().node_distro().extension;

/// A full Node version including not just the version of Node itself
/// but also the specific version of npm installed globally with that
/// Node installation.
#[derive(Clone, Debug)]
pub struct NodeVersion {
    /// The version of Node itself.
    pub runtime: Version,
    /// The npm version globally installed with the Node distro.
    pub npm: Version,
}

impl Display for NodeVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} (with {})",
            tool_version("node", &self.runtime),
            tool_version("npm", &self.npm)
        )
    }
}

/// The Tool implementation for fetching and installing Node
pub struct Node {
    pub(super) version: Version,
}

impl Node {
    pub fn new(version: Version) -> Self {
        Node { version }
    }

    pub fn archive_basename(version: &Version) -> String {
        HostPlatform::current()
            .node_distro()
            .archive_basename(version)
    }

    pub fn archive_filename(version: &Version) -> String {
        HostPlatform::current()
            .node_distro()
            .archive_filename(version)
    }

    pub(crate) fn ensure_fetched(&self, session: &mut Session) -> Fallible<NodeVersion> {
        match check_fetched(|| node_available(&self.version))? {
            FetchStatus::AlreadyFetched => {
                debug_already_fetched(self);
                let npm = fetch::load_default_npm_version(&self.version)?;

                Ok(NodeVersion {
                    runtime: self.version.clone(),
                    npm,
                })
            }
            FetchStatus::FetchNeeded(_lock) => fetch::fetch(&self.version, session.hooks()?.node()),
        }
    }
}

impl Tool for Node {
    fn fetch(self: Box<Self>, session: &mut Session) -> Fallible<()> {
        let node_version = self.ensure_fetched(session)?;

        info_fetched(node_version);
        Ok(())
    }
    fn install(self: Box<Self>, session: &mut Session) -> Fallible<()> {
        // Acquire a lock on the Volta directory, if possible, to prevent concurrent changes
        let _lock = VoltaLock::acquire();
        let node_version = self.ensure_fetched(session)?;

        let default_toolchain = session.toolchain_mut()?;
        default_toolchain.set_active_node(&self.version)?;

        // If the user has a default version of `npm`, we shouldn't show the "(with npm@X.Y.ZZZ)" text in the success message
        // Instead we should check if the bundled version is higher than the default and inform the user
        // Note: The previous line ensures that there will be a default platform
        if let Some(default_npm) = &default_toolchain.platform().unwrap().npm {
            info_installed(&self); // includes node version

            if node_version.npm > *default_npm {
                info!("{} this version of Node includes {}, which is higher than your default version ({}).
      To use the version included with Node, run `volta install npm@bundled`",
                    note_prefix(),
                    tool_version("npm", node_version.npm),
                    default_npm.to_string()
                );
            }
        } else {
            info_installed(node_version); // includes node and npm version
        }

        check_shim_reachable("node");

        if let Ok(Some(project)) = session.project_platform() {
            info_project_version(tool_version("node", &project.node), &self);
        }

        Ok(())
    }
    fn pin(self: Box<Self>, session: &mut Session) -> Fallible<()> {
        if session.project()?.is_some() {
            let node_version = self.ensure_fetched(session)?;

            // Note: We know this will succeed, since we checked above
            let project = session.project_mut()?.unwrap();
            project.pin_node(self.version.clone())?;

            // If the user has a pinned version of `npm`, we shouldn't show the "(with npm@X.Y.ZZZ)" text in the success message
            // Instead we should check if the bundled version is higher than the pinned and inform the user
            // Note: The pin operation guarantees there will be a platform
            if let Some(pinned_npm) = &project.platform().unwrap().npm {
                info_pinned(self); // includes node version

                if node_version.npm > *pinned_npm {
                    info!("{} this version of Node includes {}, which is higher than your pinned version ({}).
      To use the version included with Node, run `volta pin npm@bundled`",
                        note_prefix(),
                        tool_version("npm", node_version.npm),
                        pinned_npm.to_string()
                    );
                }
            } else {
                info_pinned(node_version); // includes node and npm version
            }

            Ok(())
        } else {
            Err(ErrorKind::NotInPackage.into())
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&tool_version("node", &self.version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_distro_mapping() {
        let cases = [
            (
                HostPlatform::WindowsX86,
                ("win", "x86", "zip", "win-x86-zip", None),
            ),
            (
                HostPlatform::WindowsX64,
                ("win", "x64", "zip", "win-x64-zip", None),
            ),
            (
                HostPlatform::WindowsArm64,
                (
                    "win",
                    "arm64",
                    "zip",
                    "win-arm64-zip",
                    Some(("x64", "win-x64-zip", 20)),
                ),
            ),
            (
                HostPlatform::MacosX64,
                ("darwin", "x64", "tar.gz", "osx-x64-tar", None),
            ),
            (
                HostPlatform::MacosArm64,
                (
                    "darwin",
                    "arm64",
                    "tar.gz",
                    "osx-arm64-tar",
                    Some(("x64", "osx-x64-tar", 16)),
                ),
            ),
            (
                HostPlatform::LinuxX64,
                ("linux", "x64", "tar.gz", "linux-x64", None),
            ),
            (
                HostPlatform::LinuxArm64,
                ("linux", "arm64", "tar.gz", "linux-arm64", None),
            ),
            (
                HostPlatform::LinuxArm,
                ("linux", "armv7l", "tar.gz", "linux-armv7l", None),
            ),
        ];

        for (platform, (os, arch, extension, identifier, fallback)) in cases {
            let distro = platform.node_distro();
            assert_eq!(distro.os, os);
            assert_eq!(distro.arch, arch);
            assert_eq!(distro.extension, extension);
            assert_eq!(distro.identifier, identifier);
            assert_eq!(
                distro.fallback.map(|fallback| (
                    fallback.arch,
                    fallback.identifier,
                    fallback.min_major
                )),
                fallback
            );
        }
    }

    #[test]
    fn archive_basename_uses_primary_arch() {
        let version = Version::parse("20.2.3").unwrap();

        for platform in HostPlatform::ALL {
            let distro = platform.node_distro();
            assert_eq!(
                distro.archive_basename(&version),
                format!("node-v20.2.3-{}-{}", distro.os, distro.arch)
            );
        }
    }

    #[test]
    fn archive_filename_uses_primary_arch() {
        let version = Version::parse("20.2.3").unwrap();

        for platform in HostPlatform::ALL {
            let distro = platform.node_distro();
            assert_eq!(
                distro.archive_filename(&version),
                format!(
                    "node-v20.2.3-{}-{}.{}",
                    distro.os, distro.arch, distro.extension
                )
            );
        }
    }

    #[test]
    fn macos_arm64_falls_back_before_node_16() {
        let distro = HostPlatform::MacosArm64.node_distro();
        let version = Version::parse("15.2.3").unwrap();

        assert_eq!(distro.archive_basename(&version), "node-v15.2.3-darwin-x64");
        assert_eq!(
            distro.archive_filename(&version),
            "node-v15.2.3-darwin-x64.tar.gz"
        );
        assert_eq!(
            distro.archive_basename(&Version::parse("16.0.0").unwrap()),
            "node-v16.0.0-darwin-arm64"
        );
    }

    #[test]
    fn windows_arm64_falls_back_before_node_20() {
        let distro = HostPlatform::WindowsArm64.node_distro();
        let version = Version::parse("19.2.3").unwrap();

        assert_eq!(distro.archive_basename(&version), "node-v19.2.3-win-x64");
        assert_eq!(
            distro.archive_filename(&version),
            "node-v19.2.3-win-x64.zip"
        );
        assert_eq!(
            distro.archive_basename(&Version::parse("20.0.0").unwrap()),
            "node-v20.0.0-win-arm64"
        );
    }

    #[test]
    fn supports_primary_and_fallback_identifiers() {
        let macos = HostPlatform::MacosArm64.node_distro();
        let fallback = HashSet::from(["osx-x64-tar".to_string()]);

        assert!(macos.supports(&HashSet::from(["osx-arm64-tar".to_string()])));
        assert!(macos.supports(&fallback));
        assert!(!macos.supports(&HashSet::from(["linux-x64".to_string()])));

        let linux = HostPlatform::LinuxX64.node_distro();
        assert!(linux.supports(&HashSet::from(["linux-x64".to_string()])));
        assert!(!linux.supports(&fallback));
    }

    #[test]
    fn identifies_macos_platforms() {
        for platform in HostPlatform::ALL {
            assert_eq!(
                platform.is_macos(),
                matches!(platform, HostPlatform::MacosX64 | HostPlatform::MacosArm64)
            );
        }
    }

    #[test]
    fn current_platform_constants_match_current_distro() {
        let distro = HostPlatform::current().node_distro();

        assert_eq!(NODE_DISTRO_OS, distro.os);
        assert_eq!(NODE_DISTRO_ARCH, distro.arch);
        assert_eq!(NODE_DISTRO_EXTENSION, distro.extension);
    }

    #[test]
    fn node_archive_uses_current_platform() {
        let version = Version::parse("20.2.3").unwrap();
        let distro = HostPlatform::current().node_distro();

        assert_eq!(
            Node::archive_basename(&version),
            distro.archive_basename(&version)
        );
        assert_eq!(
            Node::archive_filename(&version),
            distro.archive_filename(&version)
        );
    }
}
