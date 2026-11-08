//! Logic for uninstalling runtime tools (Node, npm, pnpm, Yarn) by removing
//! their locally cached versions from the inventory.
//!
//! Uninstalling is a purely local operation: the requested tool spec is matched
//! against the versions cached on the local machine, without consulting any
//! remote index.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::error::{ErrorKind, Fallible, VoltaError};
use crate::fs::{remove_dir_if_exists, remove_file_if_exists};
use crate::inventory;
use crate::layout::volta_home;
use crate::platform::PlatformSpec;
use crate::session::Session;
use crate::style::success_prefix;
use crate::sync::VoltaLock;
use crate::tool::node::HostPlatform;
use crate::tool::package::PackageConfig;
use crate::tool::{Node, Npm, Pnpm, Yarn};
use crate::version::VersionSpec;
use log::{info, warn};
use node_semver::Version;

/// The runtime tools that support being uninstalled
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeTool {
    Node,
    Npm,
    Pnpm,
    Yarn,
}

impl RuntimeTool {
    /// The name of the tool, as users write it in commands
    fn name(self) -> &'static str {
        match self {
            RuntimeTool::Node => "node",
            RuntimeTool::Npm => "npm",
            RuntimeTool::Pnpm => "pnpm",
            RuntimeTool::Yarn => "yarn",
        }
    }

    /// The path to the cached archive for the given version
    fn archive_path(self, version: &Version) -> Fallible<PathBuf> {
        let home = volta_home()?;
        let dir = match self {
            RuntimeTool::Node => home.node_inventory_dir(),
            RuntimeTool::Npm => home.npm_inventory_dir(),
            RuntimeTool::Pnpm => home.pnpm_inventory_dir(),
            RuntimeTool::Yarn => home.yarn_inventory_dir(),
        };
        Ok(dir.join(self.archive_filename(version)))
    }

    /// The filename of the cached archive for the given version
    fn archive_filename(self, version: &Version) -> String {
        match self {
            RuntimeTool::Node => Node::archive_filename(version),
            RuntimeTool::Npm => Npm::archive_filename(&version.to_string()),
            RuntimeTool::Pnpm => Pnpm::archive_filename(&version.to_string()),
            RuntimeTool::Yarn => Yarn::archive_filename(&version.to_string()),
        }
    }

    /// Collects the set of all versions of this tool with a cached image on the local machine
    fn cached_versions(self) -> Fallible<BTreeSet<Version>> {
        match self {
            RuntimeTool::Node => inventory::node_versions(),
            RuntimeTool::Npm => inventory::npm_versions(),
            RuntimeTool::Pnpm => inventory::pnpm_versions(),
            RuntimeTool::Yarn => inventory::yarn_versions(),
        }
    }
}

/// Uninstall the locally cached versions of a runtime tool that match the given spec
pub(super) fn uninstall_runtime(
    tool: RuntimeTool,
    spec: VersionSpec,
    session: &mut Session,
) -> Fallible<()> {
    // Tag-based specs (e.g. `node@latest`) would need to be resolved against a
    // remote index, which uninstall never consults
    if let VersionSpec::Tag(tag) = &spec {
        return Err(ErrorKind::UninstallTagUnsupported {
            tool: tool.name().to_owned(),
            tag: tag.to_string(),
        }
        .into());
    }

    // Acquire a lock on the Volta directory to prevent concurrent changes.
    // Removal is destructive, so do not proceed if the lock cannot be acquired
    let _lock = VoltaLock::acquire()?;

    let matching: Vec<Version> = tool
        .cached_versions()?
        .into_iter()
        .filter(|version| matches_spec(&spec, version))
        .collect();

    if matching.is_empty() {
        warn!(
            "No cached versions of '{}' found to uninstall",
            spec_label(tool, &spec)
        );
        return Ok(());
    }

    let exact = matches!(spec, VersionSpec::Exact(_));
    let (in_use, error) = InUseVersions::collect(session);

    // In-use detection is best-effort for explicit removals: a malformed
    // project manifest or package config should not block a version the
    // user named exactly, though the references that could still be read
    // are preserved for the warnings. Broad removals stay strict, since
    // the discovered references are what protect those versions.
    if let Some(err) = error {
        if !exact {
            return Err(err);
        }
        warn!(
            "Unable to fully determine in-use versions ({}); removing '{}' anyway",
            err,
            spec_label(tool, &spec)
        );
    }

    for version in matching {
        let versioned = format!("{}@{}", tool.name(), version);
        let reasons = in_use.reasons_for(tool, &version);

        // In-use versions are only removed when explicitly targeted by exact version
        if !reasons.is_empty() && !exact {
            warn!(
                "skipping '{}' ({}; target it with an exact version to remove it)",
                versioned,
                reasons.skip_phrases(tool).join("; ")
            );
            continue;
        }

        remove_version(tool, &version)?;
        info!("{} '{}' uninstalled", success_prefix(), versioned);

        for message in reasons.removal_warnings(tool, &versioned) {
            warn!("{}", message);
        }
    }

    Ok(())
}

/// Removes all locally cached copies of a version of a tool:
/// the cached archive(s), the unpacked image, and any associated metadata files
///
/// The removal is ordered so that any failure leaves a state that is either
/// consistent or retryable:
///
/// - The archive is removed first; if a later step fails, the image still
///   matches on retry
/// - The image is removed before the bundled npm metadata file: an image
///   without its metadata is treated as cached but fails to load, while a
///   leftover metadata file is harmlessly overwritten by a re-fetch
fn remove_version(tool: RuntimeTool, version: &Version) -> Fallible<()> {
    let home = volta_home()?;
    let version_str = version.to_string();

    if tool == RuntimeTool::Node {
        // Node distro archives are host-platform-specific; remove every
        // variant of the version, e.g. one fetched by a Volta binary built
        // for a different architecture
        for platform in HostPlatform::ALL {
            remove_file_if_exists(
                home.node_inventory_dir()
                    .join(platform.node_distro().archive_filename(version)),
            )?;
        }
    } else {
        let archive = tool.archive_path(version)?;
        remove_file_if_exists(archive)?;
    }

    let image_dir = match tool {
        RuntimeTool::Node => home.node_image_dir(&version_str),
        RuntimeTool::Npm => home.npm_image_dir(&version_str),
        RuntimeTool::Pnpm => home.pnpm_image_dir(&version_str),
        RuntimeTool::Yarn => home.yarn_image_dir(&version_str),
    };
    remove_dir_if_exists(image_dir)?;

    // Remove the bundled npm metadata file for the removed node version, if
    // any; last, since a remaining image is only usable with its metadata
    if tool == RuntimeTool::Node {
        remove_file_if_exists(home.node_npm_version_file(&version_str))?;
    }

    Ok(())
}

/// Determines whether a cached version matches the requested spec
fn matches_spec(spec: &VersionSpec, version: &Version) -> bool {
    match spec {
        // A bare tool name matches every cached version
        VersionSpec::None => true,
        VersionSpec::Semver(requirements) => requirements.satisfies(version),
        VersionSpec::Exact(exact) => exact == version,
        // Tag-based specs are rejected before matching
        VersionSpec::Tag(_) => false,
    }
}

/// Generates the label for a tool spec, as shown in messages
fn spec_label(tool: RuntimeTool, spec: &VersionSpec) -> String {
    match spec {
        VersionSpec::None => tool.name().to_owned(),
        other => format!("{}@{}", tool.name(), other),
    }
}

/// The versions of each tool that are currently in use, either as the user's
/// default, pinned in the current project, or depended on by an installed
/// global package
struct InUseVersions {
    default: Option<PlatformSpec>,
    project: Option<PlatformSpec>,
    packages: Vec<PackageConfig>,
}

impl InUseVersions {
    /// Collects the versions of each tool that are currently in use.
    ///
    /// Returns the references that could be read alongside the first error
    /// encountered, so that callers can decide whether partial information
    /// is acceptable.
    fn collect(session: &mut Session) -> (Self, Option<VoltaError>) {
        let mut error = None;

        let default = match session.default_platform() {
            Ok(platform) => platform.cloned(),
            Err(err) => {
                error.get_or_insert(err);
                None
            }
        };

        let project = match session.project_platform() {
            Ok(platform) => platform.cloned(),
            Err(err) => {
                error.get_or_insert(err);
                None
            }
        };

        let (configs, packages_error) = inventory::package_configs_partial();
        if let Some(err) = packages_error {
            error.get_or_insert(err);
        }
        let packages: Vec<PackageConfig> = configs.into_iter().collect();

        (
            InUseVersions {
                default,
                project,
                packages,
            },
            error,
        )
    }

    fn reasons_for(&self, tool: RuntimeTool, version: &Version) -> InUseReasons {
        InUseReasons {
            default: self
                .default
                .as_ref()
                .is_some_and(|platform| matches_version(platform, tool, version)),
            project: self
                .project
                .as_ref()
                .is_some_and(|platform| matches_version(platform, tool, version)),
            packages: self
                .packages
                .iter()
                .filter(|config| matches_version(&config.platform, tool, version))
                .map(|config| config.name.clone())
                .collect(),
        }
    }
}

/// Extracts the version of a tool from a platform spec, if any
fn spec_version(platform: &PlatformSpec, tool: RuntimeTool) -> Option<&Version> {
    match tool {
        RuntimeTool::Node => Some(&platform.node),
        RuntimeTool::Npm => platform.npm.as_ref(),
        RuntimeTool::Pnpm => platform.pnpm.as_ref(),
        RuntimeTool::Yarn => platform.yarn.as_ref(),
    }
}

fn matches_version(platform: &PlatformSpec, tool: RuntimeTool, version: &Version) -> bool {
    spec_version(platform, tool).is_some_and(|candidate| candidate == version)
}

/// The reasons why a given version of a tool is in use
struct InUseReasons {
    default: bool,
    project: bool,
    packages: Vec<String>,
}

impl InUseReasons {
    fn is_empty(&self) -> bool {
        !self.default && !self.project && self.packages.is_empty()
    }

    /// Phrases describing each reason, for use in the skip warning
    fn skip_phrases(&self, tool: RuntimeTool) -> Vec<String> {
        let mut phrases = Vec::new();
        if self.default {
            phrases.push(format!("currently your default {}", tool.name()));
        }
        if self.project {
            phrases.push("pinned in the current project".to_owned());
        }
        if !self.packages.is_empty() {
            phrases.push(packages_phrase(&self.packages));
        }
        phrases
    }

    /// Warnings to emit after removing an in-use version via exact targeting
    fn removal_warnings(&self, tool: RuntimeTool, versioned: &str) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.default {
            warnings.push(format!(
                "'{}' is your default {}; it will be re-downloaded on next use. To change your default, run `volta install {}@<version>`",
                versioned,
                tool.name(),
                tool.name()
            ));
        }
        if self.project {
            warnings.push(format!(
                "'{}' is pinned in the current project; it will be re-downloaded on next use",
                versioned
            ));
        }
        if !self.packages.is_empty() {
            let names = quoted_names(&self.packages);
            warnings.push(match names.len() {
                1 => format!(
                    "'{}' is used by global package {}; it will be re-downloaded the next time {} runs",
                    versioned, names[0], names[0]
                ),
                _ => format!(
                    "'{}' is used by global packages {}; it will be re-downloaded the next time they run",
                    versioned,
                    names.join(", ")
                ),
            });
        }
        warnings
    }
}

fn packages_phrase(packages: &[String]) -> String {
    let names = quoted_names(packages);
    match names.len() {
        1 => format!("in use by global package {}", names[0]),
        _ => format!("in use by global packages {}", names.join(", ")),
    }
}

fn quoted_names(packages: &[String]) -> Vec<String> {
    packages.iter().map(|name| format!("'{}'", name)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(s: &str) -> Version {
        s.parse().expect("valid version")
    }

    fn spec(s: &str) -> VersionSpec {
        s.parse().expect("valid version spec")
    }

    #[test]
    fn exact_spec_matches_only_that_version() {
        assert!(matches_spec(&spec("10.0.0"), &version("10.0.0")));
        assert!(!matches_spec(&spec("10.0.0"), &version("10.0.1")));
    }

    #[test]
    fn semver_spec_matches_all_versions_in_range() {
        let range_spec = spec("^10.0.0");
        assert!(matches_spec(&range_spec, &version("10.0.0")));
        assert!(matches_spec(&range_spec, &version("10.9.9")));
        assert!(!matches_spec(&range_spec, &version("11.0.0")));
    }

    #[test]
    fn bare_spec_matches_every_version() {
        assert!(matches_spec(&VersionSpec::None, &version("10.0.0")));
        assert!(matches_spec(&VersionSpec::None, &version("22.0.0")));
    }

    #[test]
    fn tag_spec_matches_nothing() {
        assert!(!matches_spec(&spec("latest"), &version("22.0.0")));
    }

    #[test]
    fn spec_label_omits_version_for_bare_tool() {
        assert_eq!(spec_label(RuntimeTool::Node, &VersionSpec::None), "node");
        assert_eq!(
            spec_label(RuntimeTool::Node, &spec("^10.0.0")),
            "node@>=10.0.0 <11.0.0-0"
        );
        assert_eq!(
            spec_label(RuntimeTool::Yarn, &spec("1.22.0")),
            "yarn@1.22.0"
        );
    }
}
