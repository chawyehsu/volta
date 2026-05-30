use std::ffi::OsStr;
use std::process::Command;

use log::debug;

use crate::error::Fallible;

pub fn create_command<E>(exe: E) -> Fallible<Command>
where
    E: AsRef<OsStr>,
{
    // Command::new(exe)
    let name = exe.as_ref();
    which::which_global(name)
        .map_err(|_| {
            crate::error::ErrorKind::BinaryNotFound {
                name: name.to_string_lossy().to_string(),
            }
            .into()
        })
        .map(Command::new)
}

/// Rebuild command against given PATH
#[cfg(unix)]
pub fn command_on_path<S: AsRef<OsStr>>(command: Command, path: S) -> Fallible<Command> {
    debug!("PATH: {}", path.as_ref().to_string_lossy());
    let mut command = command;
    command.env("PATH", path.as_ref());
    Ok(command)
}

/// Rebuild command against given PATH
///
/// On Windows, we need to explicitly use an absolute path to the executable,
/// otherwise the executable will not be located properly, even if we've set the PATH.
/// see: https://github.com/rust-lang/rust/issues/37519
///
/// This function will try to find the executable in the given path and rebuild
/// the command with the absolute path to the executable.
#[cfg(windows)]
pub fn command_on_path<S: AsRef<OsStr>>(command: Command, path: S) -> Fallible<Command> {
    debug!("PATH: {}", path.as_ref().to_string_lossy());
    let name = command.get_program();
    which::which_in_global(name, Some(&path))
        .and_then(|mut i| i.next().ok_or(which::Error::CannotFindBinaryPath))
        .map_err(|_| {
            crate::error::ErrorKind::BinaryNotFound {
                name: name.to_string_lossy().to_string(),
            }
            .into()
        })
        .map(Command::new)
        .map(|mut new_command| {
            let envs = command
                .get_envs()
                .filter_map(|(k, maybe_v)| Some(k).zip(maybe_v))
                .collect::<Vec<_>>();

            // The args will be the command name and any additional args.
            new_command.args(command.get_args());
            new_command.envs(envs);
            new_command.env("PATH", path.as_ref());
            new_command
        })
}
