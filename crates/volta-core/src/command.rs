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

/// Create a command for the given executable, searching in the provided paths
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
        // Set the PATH for the command to the provided paths, which will allow
        // the command to find its dependencies in the same path when executed.
        command.env("PATH", paths);
        command
    })
}
