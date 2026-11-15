use std::path::{Path, PathBuf};

use volta_core::error::Fallible;
use volta_core::fs;

use crate::command::self_update::fail;

#[cfg(windows)]
pub(crate) use volta_core::fs::cleanup_retired_executables;
pub(crate) use volta_core::fs::exe_path;

pub(crate) fn remove_or_retire(dst: &Path) -> Fallible<Option<PathBuf>> {
    fs::remove_or_retire(dst).map_err(|e| fail(e.to_string()))
}

pub(crate) fn replace_exe(new: &Path, old: &Path) -> Fallible<()> {
    fs::replace_exe(new, old).map_err(|e| fail(e.to_string()))
}
