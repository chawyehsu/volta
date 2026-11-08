//! Provides types for working with Volta's _inventory_, the local repository
//! of available tool versions.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::Path;

use crate::error::{Context, ErrorKind, Fallible, VoltaError};
use crate::fs::read_dir_eager;
use crate::layout::volta_home;
use crate::tool::PackageConfig;
use crate::version::parse_version;
use log::debug;
use node_semver::Version;
use walkdir::WalkDir;

/// Checks if a given Node version image is available on the local machine
pub fn node_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| {
        home.node_image_root_dir()
            .join(version.to_string())
            .exists()
    })
}

/// Collects a set of all Node versions fetched on the local machine
pub fn node_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.node_image_root_dir()))
}

/// Checks if a given npm version image is available on the local machine
pub fn npm_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| home.npm_image_dir(&version.to_string()).exists())
}

/// Collects a set of all npm versions fetched on the local machine
pub fn npm_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.npm_image_root_dir()))
}

/// Checks if a given pnpm version image is available on the local machine
pub fn pnpm_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| home.pnpm_image_dir(&version.to_string()).exists())
}

/// Collects a set of all pnpm versions fetched on the local machine
pub fn pnpm_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.pnpm_image_root_dir()))
}

/// Checks if a given Yarn version image is available on the local machine
pub fn yarn_available(version: &Version) -> Fallible<bool> {
    volta_home().map(|home| home.yarn_image_dir(&version.to_string()).exists())
}

/// Collects a set of all Yarn versions fetched on the local machine
pub fn yarn_versions() -> Fallible<BTreeSet<Version>> {
    volta_home().and_then(|home| read_versions(home.yarn_image_root_dir()))
}

/// Collects a set of all Package Configs on the local machine
pub fn package_configs() -> Fallible<BTreeSet<PackageConfig>> {
    let (configs, error) = package_configs_partial();
    match error {
        Some(err) => Err(err),
        None => Ok(configs),
    }
}

/// Collects all parseable Package Configs on the local machine, returning them
/// alongside the first error encountered, if any
///
/// Unlike `package_configs`, a malformed config does not discard the readable
/// ones, so callers can decide whether partial information is acceptable
pub fn package_configs_partial() -> (BTreeSet<PackageConfig>, Option<VoltaError>) {
    let package_dir = match volta_home() {
        Ok(home) => home.default_package_dir(),
        Err(err) => return (BTreeSet::new(), Some(err)),
    };

    let mut configs = BTreeSet::new();
    let mut error = None;

    for entry in WalkDir::new(package_dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(dir_entry) => {
                // Ignore directory entries and any files that don't have a .json extension.
                // This will prevent us from trying to parse OS-generated files as package
                // configs (e.g. `.DS_Store` on macOS)
                let extension = dir_entry.path().extension().and_then(OsStr::to_str);
                match (dir_entry.file_type().is_file(), extension) {
                    (true, Some(ext)) if ext.eq_ignore_ascii_case("json") => {
                        Some(dir_entry.into_path())
                    }
                    _ => None,
                }
            }
            Err(e) => {
                debug!("{}", e);
                None
            }
        })
    {
        match PackageConfig::from_file(entry) {
            Ok(config) => {
                configs.insert(config);
            }
            Err(err) => {
                error.get_or_insert(err);
            }
        }
    }

    (configs, error)
}

/// Reads the contents of a directory and returns the set of all versions found
/// in the directory's listing by parsing the directory names as semantic versions
fn read_versions(dir: &Path) -> Fallible<BTreeSet<Version>> {
    let contents = read_dir_eager(dir).with_context(|| ErrorKind::ReadDirError {
        dir: dir.to_owned(),
    })?;

    Ok(contents
        .filter(|(_, metadata)| metadata.is_dir())
        .filter_map(|(entry, _)| parse_version(entry.file_name().to_string_lossy()).ok())
        .collect())
}
