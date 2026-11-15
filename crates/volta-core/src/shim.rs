//! Provides utilities for modifying shims for 3rd-party executables

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use crate::error::{Context, ErrorKind, Fallible, VoltaError};
use crate::fs::read_dir_eager;
use crate::layout::volta_home;
use crate::sync::VoltaLock;
use log::{debug, warn};

pub use platform::create;

/// The shim names made available by installing a given runtime tool.
pub fn shim_names_for_tool(tool: &str) -> &'static [&'static str] {
    match tool {
        "node" => &["node", "npm", "npx"],
        "npm" => &["npm", "npx"],
        "yarn" => &["yarn", "yarnpkg"],
        "pnpm" => &["pnpm"],
        _ => &[],
    }
}

/// Ensure the shims for a tool, logging (but not failing) on errors.
///
/// Shim creation is an additive convenience, so a failure to write a shim must
/// not fail the tool installation that triggered it.
pub fn ensure_shims_for_tool(tool: &str) {
    // Refresh of a running shim on Windows retires the old copy into a `.trash`
    // directory under the shim dir. Clear it before creating shims so it does
    // not accumulate.
    #[cfg(windows)]
    if let Ok(home) = volta_home() {
        crate::fs::cleanup_retired_executables(home.shim_dir());
    }

    for name in shim_names_for_tool(tool) {
        if let Err(err) = platform::ensure_shim(name) {
            warn!("Unable to create shim `{}`: {}", name, err);
        }
    }
}

pub fn regenerate_shims_for_dir(dir: &Path) -> Fallible<()> {
    // Acquire a lock on the Volta directory, if possible, to prevent concurrent changes
    let _lock = VoltaLock::acquire();
    debug!("Rebuilding shims for directory: {}", dir.display());
    for shim_name in get_shim_list_deduped(dir)?.iter() {
        delete(shim_name)?;
        create(shim_name)?;
    }

    Ok(())
}

fn get_shim_list_deduped(dir: &Path) -> Fallible<HashSet<String>> {
    let contents = read_dir_eager(dir).with_context(|| ErrorKind::ReadDirError {
        dir: dir.to_owned(),
    })?;

    #[cfg(unix)]
    {
        let mut shims: HashSet<String> =
            contents.filter_map(platform::entry_to_shim_name).collect();
        ["node", "npm", "npx", "pnpm", "yarn", "yarnpkg"]
            .iter()
            .for_each(|s| {
                shims.insert(s.to_string());
            });
        Ok(shims)
    }

    #[cfg(windows)]
    {
        // On Windows, the default shims are installed in Program Files, so we don't need to generate them here
        Ok(contents.filter_map(platform::entry_to_shim_name).collect())
    }
}

pub fn delete(shim_name: &str) -> Fallible<()> {
    let shim = volta_home()?.shim_file(shim_name);

    #[cfg(windows)]
    platform::delete_git_bash_script(shim_name)?;

    match fs::remove_file(shim) {
        Ok(_) => Ok(()),
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(VoltaError::from_source(
                    err,
                    ErrorKind::ShimRemoveError {
                        name: shim_name.to_string(),
                    },
                ))
            }
        }
    }
}

#[cfg(unix)]
mod platform {
    //! Unix-specific shim utilities
    //!
    //! On macOS and Linux, creating a shim involves creating a symlink to the `volta-shim`
    //! executable. Additionally, filtering the shims from directory entries means looking
    //! for symlinks and ignoring the actual binaries
    use std::ffi::OsStr;
    use std::fs::{DirEntry, Metadata};
    use std::io;

    use crate::error::{ErrorKind, Fallible, VoltaError};
    use crate::fs::symlink_file;
    use crate::layout::{volta_home, volta_install};

    /// Ensure a shim exists, creating it as a symlink if necessary.
    pub fn ensure_shim(shim_name: &str) -> Fallible<()> {
        create(shim_name)
    }

    pub fn create(shim_name: &str) -> Fallible<()> {
        let executable = volta_install()?.shim_executable();
        let shim = volta_home()?.shim_file(shim_name);

        match symlink_file(executable, shim) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == io::ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Err(VoltaError::from_source(
                        err,
                        ErrorKind::ShimCreateError {
                            name: shim_name.to_string(),
                        },
                    ))
                }
            }
        }
    }

    /// Get the file stem as the shim name if the entry is a symlink
    ///
    /// Consumes a directory entry and its metadata, returning the
    /// shim name if the entry is a symlink.
    pub fn entry_to_shim_name((entry, metadata): (DirEntry, Metadata)) -> Option<String> {
        if metadata.file_type().is_symlink() {
            entry
                .path()
                .file_stem()
                .and_then(OsStr::to_str)
                .map(ToOwned::to_owned)
        } else {
            None
        }
    }
}

#[cfg(windows)]
mod platform {
    //! Windows-specific shim utilities
    //!
    //! On Windows, creating a shim involves creating a small .cmd script, rather than a symlink.
    //! This allows us to create shims without requiring administrator privileges or developer
    //! mode. Also, to support Git Bash, we create a similar script with bash syntax that doesn't
    //! have a file extension. This allows Powershell and Cmd to ignore it, while Bash detects it
    //! as an executable script.
    //!
    //! Finally, filtering directory entries to find the shim files involves looking for the .cmd
    //! files.
    use std::ffi::OsStr;
    use std::fs::{DirEntry, Metadata, write};
    use std::path::Path;

    use crate::error::{Context, ErrorKind, Fallible, VoltaError};
    use crate::fs::{file_sha256, remove_file_if_exists, replace_exe};
    use crate::layout::{volta_home, volta_install};

    const SHIM_SCRIPT_CONTENTS: &str = r#"@echo off
volta run %~n0 %*
"#;

    const GIT_BASH_SCRIPT_CONTENTS: &str = r#"#!/bin/bash
volta run "$(basename $0)" "$@""#;

    /// Ensure a tool shim exists as a copy of `volta-shim.exe`, refreshing it
    /// when the existing copy differs from the current shim binary.
    pub fn ensure_shim(shim_name: &str) -> Fallible<()> {
        let source = volta_install()?.shim_executable();
        let target = volta_home()?.windows_tool_shim_file(shim_name);

        if !shim_needs_refresh(&source, &target) {
            return Ok(());
        }

        replace_exe(&source, &target).map_err(|err| {
            VoltaError::from_source(
                err,
                ErrorKind::ShimCreateError {
                    name: shim_name.to_owned(),
                },
            )
        })
    }

    /// Whether `target` is missing or differs from `source`.
    ///
    /// Compares file sizes first so the common case avoids hashing, then falls
    /// back to a content hash when the sizes match.
    fn shim_needs_refresh(source: &Path, target: &Path) -> bool {
        let Ok(target_meta) = std::fs::metadata(target) else {
            return true;
        };
        let Ok(source_meta) = std::fs::metadata(source) else {
            return true;
        };

        if target_meta.len() != source_meta.len() {
            return true;
        }

        match (file_sha256(source), file_sha256(target)) {
            (Ok(source_hash), Ok(target_hash)) => source_hash != target_hash,
            _ => true,
        }
    }

    pub fn create(shim_name: &str) -> Fallible<()> {
        let shim = volta_home()?.shim_file(shim_name);

        write(shim, SHIM_SCRIPT_CONTENTS).with_context(|| ErrorKind::ShimCreateError {
            name: shim_name.to_owned(),
        })?;

        let git_bash_script = volta_home()?.shim_git_bash_script_file(shim_name);

        write(git_bash_script, GIT_BASH_SCRIPT_CONTENTS).with_context(|| {
            ErrorKind::ShimCreateError {
                name: shim_name.to_owned(),
            }
        })?;

        Ok(())
    }

    pub fn entry_to_shim_name((entry, _): (DirEntry, Metadata)) -> Option<String> {
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "cmd") {
            path.file_stem()
                .and_then(OsStr::to_str)
                .map(ToOwned::to_owned)
        } else {
            None
        }
    }

    pub fn delete_git_bash_script(shim_name: &str) -> Fallible<()> {
        let script_path = volta_home()?.shim_git_bash_script_file(shim_name);
        remove_file_if_exists(script_path).with_context(|| ErrorKind::ShimRemoveError {
            name: shim_name.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::shim_names_for_tool;

    #[test]
    fn maps_runtime_tools_to_their_shims() {
        assert_eq!(shim_names_for_tool("node"), &["node", "npm", "npx"]);
        assert_eq!(shim_names_for_tool("npm"), &["npm", "npx"]);
        assert_eq!(shim_names_for_tool("yarn"), &["yarn", "yarnpkg"]);
        assert_eq!(shim_names_for_tool("pnpm"), &["pnpm"]);
        assert!(shim_names_for_tool("unknown").is_empty());
    }
}
