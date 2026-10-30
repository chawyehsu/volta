use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

use volta_core::error::{ErrorKind, Fallible};

/// Move `old` to a unique name so the live name is freed mmediately, even while
/// the file is still mapped by a running process (Windows permits renaming but
/// not deleting a running image).
#[cfg(target_os = "windows")]
fn retire_exe(old: &Path) -> Fallible<()> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    const TRASH_DIR: &str = ".trash";

    let bin_dir = old.parent().ok_or_else(|| ErrorKind::SelfUpdateError {
        reason: "exe to retire has no parent directory".into(),
    })?;

    let trash_dir = bin_dir.join(TRASH_DIR);
    std::fs::create_dir_all(&trash_dir).map_err(|e| ErrorKind::SelfUpdateError {
        reason: format!(
            "failed to create trash directory {}: {}",
            trash_dir.display(),
            e
        ),
    })?;

    let mut unique_name = old
        .file_name()
        .ok_or_else(|| ErrorKind::SelfUpdateError {
            reason: "retired exe has no file name".into(),
        })?
        .to_os_string();

    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    unique_name.push(format!(".old.{pid}.{n}"));

    let retired = trash_dir.join(&unique_name);

    match std::fs::rename(old, &retired) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => {
            return Err(ErrorKind::SelfUpdateError {
                reason: format!(
                    "failed to rename {} to {}: {}",
                    old.display(),
                    retired.display(),
                    err
                ),
            }
            .into());
        }
    }
}

/// Removes the specified file, or on Windows, renames it to a retired
/// version if it's failed to be removed.
pub(crate) fn remove_or_retire(dst: &Path) -> Fallible<()> {
    #[cfg(windows)]
    {
        let dst = dst.with_extension("exe");
        std::fs::remove_file(&dst).or_else(|_| retire_exe(&dst).map(|_| ()))?;
    }
    #[cfg(not(windows))]
    {
        let _ = std::fs::remove_file(dst);
    }
    Ok(())
}

pub(crate) fn replace_exe(new: &Path, old: &Path) -> Fallible<()> {
    #[cfg(target_os = "windows")]
    let dest = old.with_extension("exe");
    #[cfg(not(target_os = "windows"))]
    let dest = old.to_path_buf();

    remove_or_retire(&dest)?;
    std::fs::copy(new, &dest).map_err(|e| ErrorKind::SelfUpdateError {
        reason: format!(
            "failed to copy {} to {}: {}",
            new.display(),
            dest.display(),
            e
        ),
    })?;

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755)).map_err(|e| {
            ErrorKind::SelfUpdateError {
                reason: format!("failed to set permissions for {}: {}", dest.display(), e),
            }
        })?;
    }

    Ok(())
}
