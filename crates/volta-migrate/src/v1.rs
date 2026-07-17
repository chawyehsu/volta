#[cfg(unix)]
use std::fs::remove_file;
use std::fs::File;
use std::path::PathBuf;

use super::empty::Empty;
use super::v0::V0;
use log::debug;
use volta_core::error::{Context, ErrorKind, Fallible, VoltaError};
#[cfg(unix)]
use volta_core::fs::{read_dir_eager, remove_file_if_exists};
use volta_layout::v1;

/// Represents a V1 Volta Layout (used by Volta v0.7.0 - v0.7.2)
///
/// Holds a reference to the V1 layout struct to support potential future migrations
pub struct V1 {
    pub home: v1::VoltaHome,
}

impl V1 {
    pub fn new(home: PathBuf) -> Self {
        V1 {
            home: v1::VoltaHome::new(home),
        }
    }

    /// Write the layout file to mark migration to V1 as complete
    ///
    /// Should only be called once all other migration steps are finished, so that we don't
    /// accidentally mark an incomplete migration as completed
    fn complete_migration(home: v1::VoltaHome) -> Fallible<Self> {
        debug!("Writing layout marker file");
        File::create(home.layout_file()).with_context(|| ErrorKind::CreateLayoutFileError {
            file: home.layout_file().to_owned(),
        })?;

        Ok(V1 { home })
    }
}

impl TryFrom<Empty> for V1 {
    type Error = VoltaError;

    fn try_from(old: Empty) -> Fallible<V1> {
        debug!("New Volta installation detected, creating fresh layout");

        let home = v1::VoltaHome::new(old.home);
        home.create().with_context(|| ErrorKind::CreateDirError {
            dir: home.root().to_owned(),
        })?;

        V1::complete_migration(home)
    }
}

impl TryFrom<V0> for V1 {
    type Error = VoltaError;

    fn try_from(old: V0) -> Fallible<V1> {
        debug!("Existing Volta installation detected, migrating from V0 layout");

        let new_home = v1::VoltaHome::new(old.home.root().to_owned());
        new_home
            .create()
            .with_context(|| ErrorKind::CreateDirError {
                dir: new_home.root().to_owned(),
            })?;

        #[cfg(unix)]
        {
            debug!("Removing unnecessary 'load.*' files");
            let root_contents =
                read_dir_eager(new_home.root()).with_context(|| ErrorKind::ReadDirError {
                    dir: new_home.root().to_owned(),
                })?;
            for (entry, _) in root_contents {
                let path = entry.path();
                if let Some(stem) = path.file_stem() {
                    if stem == "load" && path.is_file() {
                        remove_file(&path)
                            .with_context(|| ErrorKind::DeleteFileError { file: path })?;
                    }
                }
            }

            debug!("Removing old Volta binaries");

            let old_volta_bin = new_home.root().join("volta");
            remove_file_if_exists(old_volta_bin)?;

            let old_shim_bin = new_home.root().join("shim");
            remove_file_if_exists(old_shim_bin)?;
        }

        V1::complete_migration(new_home)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_empty_to_v1() {
        let dir = tempdir().unwrap();
        let empty = Empty::new(dir.path().to_owned());

        let v1: V1 = empty.try_into().unwrap();

        // V1 layout directories should exist
        assert!(v1.home.root().exists());
        assert!(v1.home.cache_dir().exists());
        assert!(v1.home.shim_dir().exists());
        assert!(v1.home.tools_dir().exists());
        assert!(v1.home.tmp_dir().exists());
        // Layout marker file should be written
        assert!(v1.home.layout_file().exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_v0_to_v1_removes_load_files() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Set up V0 artifacts: load.sh, load.zsh, old binaries
        std::fs::write(root.join("load.sh"), "#!/bin/sh").unwrap();
        std::fs::write(root.join("load.zsh"), "#!/bin/zsh").unwrap();
        std::fs::write(root.join("volta"), "binary").unwrap();
        std::fs::write(root.join("shim"), "binary").unwrap();

        let v0 = V0::new(root.to_owned());
        let v1: V1 = v0.try_into().unwrap();

        // load.* files should be removed
        assert!(!root.join("load.sh").exists());
        assert!(!root.join("load.zsh").exists());
        // Old binaries should be removed
        assert!(!root.join("volta").exists());
        assert!(!root.join("shim").exists());
        // V1 layout should be created
        assert!(v1.home.layout_file().exists());
    }
}
