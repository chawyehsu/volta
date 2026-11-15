//! Provides utilities for operating on the filesystem.

use std::fs::{self, DirEntry, File, Metadata, create_dir_all, read_dir};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Context, ErrorKind, Fallible};
use crate::layout::volta_home;
use retry::delay::Fibonacci;
use retry::{OperationResult, retry};
use tempfile::{NamedTempFile, TempDir, tempdir_in};

/// Opens a file, creating it if it doesn't exist
pub fn touch(path: &Path) -> io::Result<File> {
    if !path.is_file() {
        if let Some(basedir) = path.parent() {
            create_dir_all(basedir)?;
        }
        File::create(path)?;
    }
    File::open(path)
}

/// Removes the target directory, if it exists. If the directory doesn't exist, that is treated as
/// success.
pub fn remove_dir_if_exists<P: AsRef<Path>>(path: P) -> Fallible<()> {
    fs::remove_dir_all(&path)
        .or_else(ok_if_not_found)
        .with_context(|| ErrorKind::DeleteDirectoryError {
            directory: path.as_ref().to_owned(),
        })
}

/// Removes the target file, if it exists. If the file doesn't exist, that is treated as success.
pub fn remove_file_if_exists<P: AsRef<Path>>(path: P) -> Fallible<()> {
    fs::remove_file(&path)
        .or_else(ok_if_not_found)
        .with_context(|| ErrorKind::DeleteFileError {
            file: path.as_ref().to_owned(),
        })
}

/// Converts a failure because of file not found into a success.
///
/// Handling the error is preferred over checking if a file exists before removing it, since
/// that avoids a potential race condition between the check and the removal.
pub fn ok_if_not_found<T: Default>(err: io::Error) -> io::Result<T> {
    match err.kind() {
        io::ErrorKind::NotFound => Ok(T::default()),
        _ => Err(err),
    }
}

/// Reads a file, if it exists.
pub fn read_file<P: AsRef<Path>>(path: P) -> io::Result<Option<String>> {
    let result: io::Result<String> = fs::read_to_string(path);

    match result {
        Ok(string) => Ok(Some(string)),
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(error),
        },
    }
}

/// Reads the full contents of a directory, eagerly extracting each directory entry
/// and its metadata and returning an iterator over them. Returns `Error` if any of
/// these steps fails.
///
/// This function makes it easier to write high level logic for manipulating the
/// contents of directories (map, filter, etc).
///
/// Note that this function allocates an intermediate vector of directory entries to
/// construct the iterator from, so if a directory is expected to be very large, it
/// will allocate temporary data proportional to the number of entries.
pub fn read_dir_eager(dir: &Path) -> io::Result<impl Iterator<Item = (DirEntry, Metadata)>> {
    let entries = read_dir(dir)?;
    let vec = entries
        .map(|entry| {
            let entry = entry?;
            let metadata = entry.metadata()?;
            Ok((entry, metadata))
        })
        .collect::<io::Result<Vec<(DirEntry, Metadata)>>>()?;

    Ok(vec.into_iter())
}

/// Reads the contents of a directory and returns a Vec of the matched results
/// from the input function
pub fn dir_entry_match<T, F>(dir: &Path, mut f: F) -> io::Result<Vec<T>>
where
    F: FnMut(&DirEntry) -> Option<T>,
{
    let entries = read_dir_eager(dir)?;
    Ok(entries
        .filter(|(_, metadata)| metadata.is_file())
        .filter_map(|(entry, _)| f(&entry))
        .collect::<Vec<T>>())
}

/// Creates a NamedTempFile in the Volta tmp directory
pub fn create_staging_file() -> Fallible<NamedTempFile> {
    let tmp_dir = volta_home()?.tmp_dir();
    NamedTempFile::new_in(tmp_dir).with_context(|| ErrorKind::CreateTempFileError {
        in_dir: tmp_dir.to_owned(),
    })
}

/// Creates a staging directory in the Volta tmp directory
pub fn create_staging_dir() -> Fallible<TempDir> {
    let tmp_root = volta_home()?.tmp_dir();
    tempdir_in(tmp_root).with_context(|| ErrorKind::CreateTempDirError {
        in_dir: tmp_root.to_owned(),
    })
}

/// Create a file symlink. The `dst` path will be a symbolic link pointing to the `src` path.
pub fn symlink_file<S, D>(src: S, dest: D) -> io::Result<()>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(src, dest);

    #[cfg(unix)]
    return std::os::unix::fs::symlink(src, dest);
}

/// Create a directory symlink. The `dst` path will be a symbolic link pointing to the `src` path
pub fn symlink_dir<S, D>(src: S, dest: D) -> io::Result<()>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    #[cfg(windows)]
    return junction::create(src, dest);

    #[cfg(unix)]
    return std::os::unix::fs::symlink(src, dest);
}

/// Ensure that a given file has 'executable' permissions, otherwise we won't be able to call it
#[cfg(unix)]
pub fn set_executable(bin: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(bin)?.permissions();
    let mode = permissions.mode();

    if mode & 0o111 != 0o111 {
        permissions.set_mode(mode | 0o111);
        fs::set_permissions(bin, permissions)
    } else {
        Ok(())
    }
}

/// Ensure that a given file has 'executable' permissions, otherwise we won't be able to call it
///
/// Note: This is a no-op on Windows, which has no concept of 'executable' permissions
#[cfg(windows)]
pub fn set_executable(_bin: &Path) -> io::Result<()> {
    Ok(())
}

/// Creates the parent directory of the input path, assuming the input path is a file.
pub fn ensure_containing_dir_exists<P: AsRef<Path>>(path: &P) -> io::Result<()> {
    path.as_ref()
        .parent()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Could not determine directory information for {}",
                    path.as_ref().display()
                ),
            )
        })
        .and_then(fs::create_dir_all)
}

/// Rename a file or directory to a new name, retrying if the operation fails because of permissions
///
/// Will retry for ~30 seconds with longer and longer delays between each, to allow for virus scan
/// and other automated operations to complete.
pub fn rename<F, T>(from: F, to: T) -> io::Result<()>
where
    F: AsRef<Path>,
    T: AsRef<Path>,
{
    // 21 Fibonacci steps starting at 1 ms is ~28 seconds total
    // See https://github.com/rust-lang/rustup/pull/1873 where this was used by Rustup to work around
    // virus scanning file locks
    let from = from.as_ref();
    let to = to.as_ref();

    retry(Fibonacci::from_millis(1).take(21), || {
        match fs::rename(from, to) {
            Ok(_) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied => OperationResult::Retry(e),
                _ => OperationResult::Err(e),
            },
        }
    })
    .map_err(|e| e.error)
}

/// Append the platform executable suffix to a path, if necessary.
pub fn exe_path(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        path.with_extension("exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

/// Compute the SHA-256 digest of a file, reading it in chunks.
pub fn file_sha256(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

#[cfg(any(windows, test))]
const TRASH_DIR: &str = ".trash";

/// The shared trash directory, under the Volta home tmp directory.
///
/// Retired executables from both self-update and shim refresh are moved here so
/// they do not accumulate next to the binaries. Renaming only works within a
/// single volume, so callers fall back to a per-directory trash when this one is
/// on a different volume.
#[cfg(target_os = "windows")]
fn shared_trash_dir() -> Option<PathBuf> {
    crate::layout::volta_home()
        .ok()
        .map(|home| home.tmp_dir().join(TRASH_DIR))
}

/// Remove retired executables from the shared trash directory.
#[cfg(windows)]
pub fn cleanup_shared_trash() {
    if let Some(trash_dir) = shared_trash_dir() {
        cleanup_trash_dir(&trash_dir);
    }
}

/// Move `old` to a unique name so the live name is freed immediately, even while
/// the file is still mapped by a running process (Windows permits renaming but
/// not deleting a running image).
#[cfg(target_os = "windows")]
fn retire_exe(old: &Path) -> io::Result<Option<PathBuf>> {
    let unique_name = unique_retired_name(old)?;
    retire_exe_with_trash(old, shared_trash_dir().as_deref(), &unique_name)
}

/// Build the unique file name used for a retired executable.
#[cfg(target_os = "windows")]
fn unique_retired_name(old: &Path) -> io::Result<std::ffi::OsString> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let mut unique_name = old
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "retired exe has no file name"))?
        .to_os_string();

    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    unique_name.push(format!(".old.{pid}.{n}"));

    Ok(unique_name)
}

/// Retire `old` into the shared trash directory, falling back to a trash
/// directory beside the source when the shared one cannot be used (for example
/// when it lives on another volume).
#[cfg(any(windows, test))]
fn retire_exe_with_trash(
    old: &Path,
    shared_trash: Option<&Path>,
    unique_name: &std::ffi::OsStr,
) -> io::Result<Option<PathBuf>> {
    // Prefer the shared trash directory. If it lives on another volume, the
    // rename fails and we fall back to a trash directory beside the source.
    if let Some(shared) = shared_trash
        && create_dir_all(shared).is_ok()
    {
        match rename_into_trash(old, shared, unique_name) {
            Ok(retired) => return Ok(retired),
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => {}
        }
    }

    let bin_dir = old.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "exe to retire has no parent directory",
        )
    })?;
    let trash_dir = bin_dir.join(TRASH_DIR);
    create_dir_all(&trash_dir).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "failed to create trash directory {}: {}",
                trash_dir.display(),
                e
            ),
        )
    })?;

    rename_into_trash(old, &trash_dir, unique_name)
}

/// Rename `old` into `trash_dir`, returning the retired path (or `None` if the
/// source no longer exists).
#[cfg(any(windows, test))]
fn rename_into_trash(
    old: &Path,
    trash_dir: &Path,
    unique_name: &std::ffi::OsStr,
) -> io::Result<Option<PathBuf>> {
    let retired = trash_dir.join(unique_name);
    match fs::rename(old, &retired) {
        Ok(()) => Ok(Some(retired)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(io::Error::new(
            err.kind(),
            format!(
                "failed to rename {} to {}: {}",
                old.display(),
                retired.display(),
                err
            ),
        )),
    }
}

/// Removes the specified file, or on Windows, renames it to a retired
/// version if it fails to be removed.
pub fn remove_or_retire(dst: &Path) -> io::Result<Option<PathBuf>> {
    #[cfg(windows)]
    {
        let dst = exe_path(dst);
        match fs::remove_file(&dst) {
            Ok(()) => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                let is_file = fs::symlink_metadata(&dst)
                    .map(|metadata| metadata.file_type().is_file())
                    .unwrap_or(false);
                if is_file {
                    retire_exe(&dst)
                } else {
                    Err(io::Error::new(
                        error.kind(),
                        format!("failed to remove {}: {}", dst.display(), error),
                    ))
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        match fs::remove_file(dst) {
            Ok(()) => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io::Error::new(
                error.kind(),
                format!("failed to remove {}: {}", dst.display(), error),
            )),
        }
    }
}

/// Remove any executables that were retired during a previous replacement.
#[cfg(windows)]
pub fn cleanup_retired_executables(install: &Path) {
    cleanup_trash_dir(&install.join(TRASH_DIR));
}

/// Remove the files inside a trash directory, ignoring any that are still in use.
#[cfg(windows)]
fn cleanup_trash_dir(trash_dir: &Path) {
    let Ok(entries) = fs::read_dir(trash_dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Atomically replace `old` with a copy of `new`.
///
/// On Windows, the existing executable is retired (renamed) first so that a
/// running image does not block the replacement.
pub fn replace_exe(new: &Path, old: &Path) -> io::Result<()> {
    let dest = exe_path(old);
    let parent = dest.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("destination {} has no parent directory", dest.display()),
        )
    })?;
    let staged = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("failed to stage executable for {}: {}", dest.display(), e),
        )
    })?;
    fs::copy(new, staged.path()).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "failed to copy {} to {}: {}",
                new.display(),
                staged.path().display(),
                e
            ),
        )
    })?;

    #[cfg(not(target_os = "windows"))]
    {
        fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o755)).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "failed to set permissions for {}: {}",
                    staged.path().display(),
                    e
                ),
            )
        })?;
    }

    #[cfg(target_os = "windows")]
    let retired = remove_or_retire(&dest)?;

    match staged.persist(&dest) {
        Ok(_) => Ok(()),
        Err(error) => {
            #[cfg(target_os = "windows")]
            if let Some(retired) = retired {
                let _ = fs::rename(&retired, &dest);
            }
            Err(io::Error::new(
                error.error.kind(),
                format!(
                    "failed to replace {} with {}: {}",
                    dest.display(),
                    new.display(),
                    error.error
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;

    use tempfile::tempdir;

    use super::retire_exe_with_trash;

    fn source_file(dir: &std::path::Path) -> std::path::PathBuf {
        let bin_dir = dir.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let old = bin_dir.join("node.exe");
        fs::write(&old, b"old").unwrap();
        old
    }

    #[test]
    fn retire_uses_shared_trash_when_available() {
        let dir = tempdir().unwrap();
        let old = source_file(dir.path());
        let shared = dir.path().join("shared-trash");

        let retired = retire_exe_with_trash(&old, Some(&shared), OsStr::new("node.exe.old.1.0"))
            .unwrap()
            .unwrap();

        assert_eq!(retired, shared.join("node.exe.old.1.0"));
        assert!(retired.exists());
        assert!(!old.exists());
    }

    #[test]
    fn retire_falls_back_when_shared_trash_unusable() {
        let dir = tempdir().unwrap();
        let old = source_file(dir.path());

        // A file where the shared trash directory should be makes
        // `create_dir_all` fail, forcing the fallback.
        let shared = dir.path().join("shared-trash");
        fs::write(&shared, b"not a directory").unwrap();

        let retired = retire_exe_with_trash(&old, Some(&shared), OsStr::new("node.exe.old.1.0"))
            .unwrap()
            .unwrap();

        assert_eq!(
            retired,
            dir.path()
                .join("bin")
                .join(".trash")
                .join("node.exe.old.1.0")
        );
        assert!(retired.exists());
        assert!(!old.exists());
    }

    #[test]
    fn retire_falls_back_when_shared_rename_fails() {
        let dir = tempdir().unwrap();
        let old = source_file(dir.path());

        // Pre-create the destination as a directory so the rename into the
        // shared trash fails, forcing the fallback.
        let shared = dir.path().join("shared-trash");
        let blocker = shared.join("node.exe.old.1.0");
        fs::create_dir_all(blocker.join("sub")).unwrap();

        let retired = retire_exe_with_trash(&old, Some(&shared), OsStr::new("node.exe.old.1.0"))
            .unwrap()
            .unwrap();

        assert_eq!(
            retired,
            dir.path()
                .join("bin")
                .join(".trash")
                .join("node.exe.old.1.0")
        );
        assert!(retired.exists());
        assert!(!old.exists());
    }
}
