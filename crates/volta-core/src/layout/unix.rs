use std::path::PathBuf;

use super::volta_home;
use crate::error::{ErrorKind, Fallible};

/// Returns the default path to the Volta home directory within
/// the user's home directory, i.e., `~/.volta`.
pub(super) fn default_home_dir() -> Fallible<PathBuf> {
    let mut home = dirs::home_dir().ok_or(ErrorKind::NoHomeEnvironmentVar)?;
    home.push(".volta");
    Ok(home)
}

pub fn env_paths() -> Fallible<Vec<PathBuf>> {
    let home = volta_home()?;
    Ok(vec![home.shim_dir().to_owned()])
}
