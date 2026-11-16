use std::env;
use std::path::PathBuf;

use crate::constant;
use crate::error::{Context, ErrorKind, Fallible};
use cfg_if::cfg_if;
use dunce::canonicalize;
use once_cell::sync::OnceCell;
use volta_layout::v4::{VoltaHome, VoltaInstall};

cfg_if! {
    if #[cfg(unix)] {
        mod unix;
        use unix::default_home_dir;
        pub use unix::env_paths;
    } else if #[cfg(windows)] {
        mod windows;
        use windows::default_home_dir;
        pub use windows::env_paths;
    }
}

static VOLTA_HOME: OnceCell<VoltaHome> = OnceCell::new();
static VOLTA_INSTALL: OnceCell<VoltaInstall> = OnceCell::new();

/// Returns the Volta home directory.
///
/// This is determined either from the `VOLTA_HOME` environment variable
/// or by using the default home directory.
pub fn volta_home<'a>() -> Fallible<&'a VoltaHome> {
    VOLTA_HOME.get_or_try_init(|| {
        let home_dir = match env::var_os(constant::ENVNAME_VOLTA_HOME) {
            Some(home) => PathBuf::from(home),
            None => default_home_dir()?,
        };

        Ok(VoltaHome::new(home_dir))
    })
}

/// Returns the Volta install directory.
///
/// This is determined either from the `VOLTA_INSTALL_DIR` environment
/// variable or by using the default install directory based on the
/// location of the currently running executable.
pub fn volta_install<'a>() -> Fallible<&'a VoltaInstall> {
    VOLTA_INSTALL.get_or_try_init(|| {
        let install_dir = match env::var_os(constant::ENVNAME_VOLTA_INSTALL_DIR) {
            Some(install) => PathBuf::from(install),
            None => default_install_dir()?,
        };

        Ok(VoltaInstall::new(install_dir))
    })
}

/// Determine the binary install directory from the currently running executable
///
/// The volta-shim and volta binaries will be installed in the same location, so we can use the
/// currently running executable to find the binary install directory. Note that we need to
/// canonicalize the path we get from current_exe to make sure we resolve symlinks and find the
/// actual binary files.
///
/// On Windows, on-demand tool shims are copies of `volta-shim.exe` in the Volta home bin
/// directory rather than symlinks, so their own directory is not the install directory. In that
/// case the real install directory is resolved from the `volta` executable on `PATH`.
fn default_install_dir() -> Fallible<PathBuf> {
    let exe = env::current_exe()
        .and_then(canonicalize)
        .with_context(|| ErrorKind::NoInstallDir)?;
    let dir = exe
        .parent()
        .map(|parent| parent.to_owned())
        .ok_or(ErrorKind::NoInstallDir)?;

    // On Windows, on-demand tool shims are copies of `volta-shim.exe` placed in
    // the Volta home bin directory. When one of those copies is running, its own
    // directory is not the Volta install directory, so resolve the real install
    // directory from the `volta` executable on PATH instead.
    #[cfg(windows)]
    if let Ok(home) = volta_home()
        && let Ok(shim_dir) = canonicalize(home.shim_dir())
        && shim_dir == dir
        && let Ok(volta) = which::which("volta")
        && let Ok(volta) = canonicalize(&volta)
        && let Some(parent) = volta.parent()
    {
        return Ok(parent.to_owned());
    }

    Ok(dir)
}
