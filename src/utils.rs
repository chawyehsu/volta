use std::path::{Path, PathBuf};

use volta_core::error::Fallible;

use crate::command::self_update::fail;

pub(crate) fn exe_path(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        path.with_extension("exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

/// Move `old` to a unique name so the live name is freed mmediately, even while
/// the file is still mapped by a running process (Windows permits renaming but
/// not deleting a running image).
#[cfg(target_os = "windows")]
fn retire_exe(old: &Path) -> Fallible<Option<PathBuf>> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    const TRASH_DIR: &str = ".trash";

    let bin_dir = old
        .parent()
        .ok_or_else(|| fail("exe to retire has no parent directory"))?;

    let trash_dir = bin_dir.join(TRASH_DIR);
    std::fs::create_dir_all(&trash_dir).map_err(|e| {
        fail(format!(
            "failed to create trash directory {}: {}",
            trash_dir.display(),
            e
        ))
    })?;

    let mut unique_name = old
        .file_name()
        .ok_or_else(|| fail("retired exe has no file name"))?
        .to_os_string();

    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    unique_name.push(format!(".old.{pid}.{n}"));

    let retired = trash_dir.join(&unique_name);

    match std::fs::rename(old, &retired) {
        Ok(()) => Ok(Some(retired)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => {
            return Err(fail(format!(
                "failed to rename {} to {}: {}",
                old.display(),
                retired.display(),
                err
            )));
        }
    }
}

/// Removes the specified file, or on Windows, renames it to a retired
/// version if it's failed to be removed.
pub(crate) fn remove_or_retire(dst: &Path) -> Fallible<Option<PathBuf>> {
    #[cfg(windows)]
    {
        let dst = exe_path(dst);
        match std::fs::remove_file(&dst) {
            Ok(()) => Ok(None),
            Err(_) => retire_exe(&dst),
        }
    }
    #[cfg(not(windows))]
    {
        match std::fs::remove_file(dst) {
            Ok(()) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(fail(format!(
                "failed to remove {}: {}",
                dst.display(),
                error
            ))),
        }
    }
}

pub(crate) fn replace_exe(new: &Path, old: &Path) -> Fallible<()> {
    let dest = exe_path(old);
    let parent = dest.parent().ok_or_else(|| {
        fail(format!(
            "destination {} has no parent directory",
            dest.display()
        ))
    })?;
    let staged = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
        fail(format!(
            "failed to stage executable for {}: {}",
            dest.display(),
            e
        ))
    })?;
    std::fs::copy(new, staged.path()).map_err(|e| {
        fail(format!(
            "failed to copy {} to {}: {}",
            new.display(),
            staged.path().display(),
            e
        ))
    })?;

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(staged.path(), std::fs::Permissions::from_mode(0o755)).map_err(
            |e| {
                fail(format!(
                    "failed to set permissions for {}: {}",
                    staged.path().display(),
                    e
                ))
            },
        )?;
    }

    #[cfg(target_os = "windows")]
    let retired = remove_or_retire(&dest)?;

    match staged.persist(&dest) {
        Ok(_) => Ok(()),
        Err(error) => {
            #[cfg(target_os = "windows")]
            if let Some(retired) = retired {
                let _ = std::fs::rename(&retired, &dest);
            }
            Err(fail(format!(
                "failed to replace {} with {}: {}",
                dest.display(),
                new.display(),
                error.error
            )))
        }
    }
}
