use std::ffi::OsStr;
use std::process::Command;

use crate::error::Fallible;

/// Look up the given executable with `lookup` and create a `Command` for it
fn command_lookup<E, F>(exe: E, lookup: F) -> Fallible<Command>
where
    E: AsRef<OsStr>,
    F: FnOnce(&OsStr) -> Result<std::path::PathBuf, which::Error>,
{
    let name = exe.as_ref();
    lookup(name).map(Command::new).map_err(|_| {
        crate::error::ErrorKind::BinaryNotFound {
            name: name.to_string_lossy().to_string(),
        }
        .into()
    })
}

/// Create a command for the given executable, searching in the default PATH
pub fn create_command<E>(exe: E) -> Fallible<Command>
where
    E: AsRef<OsStr>,
{
    command_lookup(exe, |name| which::which_global(name))
}

/// Create a command for the given executable, searching in the provided paths.
///
/// On Unix, we verify the executable exists in the given paths but pass the bare name to
/// `Command::new`. This preserves `execvp`-style PATH-search semantics at the OS level: the
/// kernel receives a name rather than an absolute path, which avoids `ENOEXEC` (os error 8)
/// that can occur when directly exec-ing certain script wrappers or zero-byte fixture files.
#[cfg(unix)]
pub fn create_command_in<E, U>(exe: E, paths: U) -> Fallible<Command>
where
    E: AsRef<OsStr>,
    U: AsRef<OsStr>,
{
    let exe = exe.as_ref();
    let paths = paths.as_ref();

    which::which_in_global(exe, Some(paths))
        .and_then(|mut iter| iter.next().ok_or(which::Error::CannotFindBinaryPath))
        .map_err(|_| {
            crate::error::ErrorKind::BinaryNotFound {
                name: exe.to_string_lossy().to_string(),
            }
            .into()
        })
        .map(|_| {
            let mut command = Command::new(exe);
            // Set PATH so command execution and nested launches resolve tools in this context.
            command.env("PATH", paths);
            command
        })
}

/// Create a command for the given executable, searching in the provided paths.
///
/// On Windows, command launching requires an absolute executable path.
#[cfg(windows)]
pub fn create_command_in<E, U>(exe: E, paths: U) -> Fallible<Command>
where
    E: AsRef<OsStr>,
    U: AsRef<OsStr>,
{
    let paths = paths.as_ref();

    command_lookup(exe, |name| {
        which::which_in_global(name, Some(paths))
            .and_then(|mut iter| iter.next().ok_or(which::Error::CannotFindBinaryPath))
    })
    .map(|mut command| {
        // Set PATH so command execution and nested launches resolve tools in this context.
        command.env("PATH", paths);
        command
    })
}
