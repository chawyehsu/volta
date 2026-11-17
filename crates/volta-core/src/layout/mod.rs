use std::env;
use std::path::{Path, PathBuf};

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

    // When Volta was installed by Homebrew (macOS or Linux), the canonicalized
    // executable lives under a versioned Cellar directory that is removed on
    // `brew upgrade`. Prefer the version-stable `opt` directory so shim
    // symlinks keep working across upgrades (see volta-cli/volta#927).
    #[cfg(unix)]
    if let Some(dir) = homebrew_stable_bin_dir(&exe) {
        return Ok(dir);
    }

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

/// Map a Homebrew Cellar executable path to its version-stable `opt` directory.
///
/// Homebrew installs formula executables under
/// `<prefix>/Cellar/<formula>/<version>/bin/` and removes that versioned
/// directory on `brew upgrade`. It maintains a stable symlink at
/// `<prefix>/opt/<formula>` that always points at the active Cellar version,
/// so symlinks created through the `opt` path keep working across upgrades.
///
/// Given a canonicalized Volta executable path, returns
/// `<prefix>/opt/<formula>/bin` when the path matches the Cellar layout
/// (`Cellar` followed by the formula, version, and `bin`, with the executable
/// directly inside), or `None` when it does not.
#[cfg(unix)]
fn cellar_opt_bin_dir(exe: &Path) -> Option<PathBuf> {
    use std::path::Component;

    const CELLAR: &str = "Cellar";
    const BIN: &str = "bin";

    let components: Vec<_> = exe.components().collect();
    let len = components.len();
    // Layout: [<prefix>...] Cellar <formula> <version> bin <executable>, where
    // the path is canonicalized and therefore absolute.
    (0..len.saturating_sub(4))
        .find(|&i| {
            len == i + 5
                && components[0] == Component::RootDir
                && components[i].as_os_str() == CELLAR
                && components[i + 3].as_os_str() == BIN
        })
        .map(|i| {
            let prefix: PathBuf = components[..i].iter().collect();
            let formula = components[i + 1].as_os_str();
            prefix.join("opt").join(formula).join(BIN)
        })
}

/// Determine a version-stable install directory for a Homebrew installation.
///
/// Returns the `opt` bin directory only when it resolves (through Homebrew's
/// `opt` symlink) to the directory containing the running executable, so that
/// the stable path is only used when it actually refers to the active Volta
/// version. Returns `None` otherwise, in which case the caller falls back to
/// the canonicalized executable directory.
#[cfg(unix)]
fn homebrew_stable_bin_dir(exe: &Path) -> Option<PathBuf> {
    let opt_bin_dir = cellar_opt_bin_dir(exe)?;
    let resolved = canonicalize(&opt_bin_dir).ok()?;
    (resolved == exe.parent()?).then_some(opt_bin_dir)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    mod unix {
        use super::super::{canonicalize, cellar_opt_bin_dir, homebrew_stable_bin_dir};
        use std::fs;
        use std::os::unix::fs::symlink;
        use std::path::{Path, PathBuf};

        /// Write a placeholder executable file.
        fn write_executable(path: &Path, contents: &str) {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }

        /// Create a Homebrew-style formula version directory tree.
        ///
        /// Returns the canonicalized path of the formula's `volta-shim`.
        fn install_version(prefix: &Path, version: &str, marker: &str) -> PathBuf {
            let bin = prefix.join("Cellar/volta").join(version).join("bin");
            let shim = bin.join("volta-shim");
            write_executable(&shim, marker);
            canonicalize(&shim).unwrap()
        }

        /// Point Homebrew's stable `opt` symlink at a formula version, using
        /// the same relative target style Homebrew does.
        fn link_opt(prefix: &Path, version: &str) {
            let link = prefix.join("opt/volta");
            fs::create_dir_all(link.parent().unwrap()).unwrap();
            fs::remove_file(&link).ok();
            symlink(format!("../Cellar/volta/{version}"), link).unwrap();
        }

        #[test]
        fn maps_cellar_bin_dir_to_stable_opt_dir() {
            let cases = [
                (
                    "/opt/homebrew/Cellar/volta/2.1.0/bin/volta-shim",
                    "/opt/homebrew/opt/volta/bin",
                ),
                (
                    "/usr/local/Cellar/volta/1.0.4/bin/volta",
                    "/usr/local/opt/volta/bin",
                ),
                (
                    "/home/linuxbrew/.linuxbrew/Cellar/volta/2.0.2_1/bin/volta-migrate",
                    "/home/linuxbrew/.linuxbrew/opt/volta/bin",
                ),
            ];

            for (exe, expected) in cases {
                assert_eq!(
                    cellar_opt_bin_dir(Path::new(exe)),
                    Some(PathBuf::from(expected)),
                    "wrong mapping for {exe}"
                );
            }
        }

        #[test]
        fn ignores_non_cellar_layouts() {
            let cases = [
                "/usr/local/bin/volta-shim",
                "/opt/homebrew/bin/volta",
                "/opt/homebrew/Cellar/volta/bin/volta-shim", // missing version
                "/opt/homebrew/Cellar/volta/2.1.0/libexec/volta-shim", // no bin directory
                "/opt/homebrew/Cellar/volta/2.1.0/bin/sub/volta-shim", // nested below bin
                "Cellar/volta/2.1.0/bin/volta-shim",         // relative, not canonical
            ];

            for exe in cases {
                assert_eq!(
                    cellar_opt_bin_dir(Path::new(exe)),
                    None,
                    "unexpected mapping for {exe}"
                );
            }
        }

        #[test]
        fn uses_opt_dir_when_it_matches_running_executable() {
            let dir = tempfile::tempdir().unwrap();
            // Canonicalize the temp dir so path comparisons work even when the
            // temp root itself is reached through a symlink (e.g. /var).
            let prefix = canonicalize(dir.path()).unwrap();
            let exe = install_version(&prefix, "2.1.0", "v2.1.0");
            link_opt(&prefix, "2.1.0");

            let stable = homebrew_stable_bin_dir(&exe).expect("opt dir should be used");
            assert_eq!(stable, prefix.join("opt/volta/bin"));
            // The stable path must resolve back to the running executable's directory.
            assert_eq!(canonicalize(&stable).unwrap(), exe.parent().unwrap());
        }

        #[test]
        fn falls_back_when_opt_dir_is_missing() {
            let dir = tempfile::tempdir().unwrap();
            let prefix = canonicalize(dir.path()).unwrap();
            let exe = install_version(&prefix, "2.1.0", "v2.1.0");

            // No `opt` symlink at all.
            assert_eq!(homebrew_stable_bin_dir(&exe), None);

            // An `opt` symlink whose target does not exist (broken link).
            fs::create_dir_all(prefix.join("opt")).unwrap();
            symlink("../Cellar/volta/9.9.9", prefix.join("opt/volta")).unwrap();
            assert_eq!(homebrew_stable_bin_dir(&exe), None);
        }

        #[test]
        fn falls_back_when_opt_points_to_another_version() {
            let dir = tempfile::tempdir().unwrap();
            let prefix = canonicalize(dir.path()).unwrap();
            let running = install_version(&prefix, "2.1.0", "v2.1.0");
            install_version(&prefix, "2.2.0", "v2.2.0");
            link_opt(&prefix, "2.2.0");

            // `opt` resolves to a different directory than the running
            // executable, so it must not be used.
            assert_eq!(homebrew_stable_bin_dir(&running), None);
        }

        #[test]
        fn shim_resolves_after_opt_switches_versions() {
            let dir = tempfile::tempdir().unwrap();
            let prefix = canonicalize(dir.path()).unwrap();
            let home = tempfile::tempdir_in(dir.path()).unwrap();

            // First install: create a shim symlink through the stable opt path.
            let exe_v1 = install_version(&prefix, "2.1.0", "v2.1.0");
            link_opt(&prefix, "2.1.0");
            let stable = homebrew_stable_bin_dir(&exe_v1).expect("opt dir should be used");
            let shim = home.path().join("node");
            symlink(stable.join("volta-shim"), &shim).unwrap();
            assert_eq!(
                canonicalize(&shim).unwrap(),
                exe_v1,
                "shim should resolve to the 2.1.0 shim binary"
            );

            // Simulate `brew upgrade`: the old Cellar version is removed, a new
            // one is installed, and Homebrew retargets the opt symlink. The
            // shim symlink itself is left untouched.
            fs::remove_file(prefix.join("opt/volta")).unwrap();
            fs::remove_dir_all(prefix.join("Cellar/volta/2.1.0")).unwrap();
            let exe_v2 = install_version(&prefix, "2.2.0", "v2.2.0");
            link_opt(&prefix, "2.2.0");

            assert!(shim.is_file(), "shim symlink broken after upgrade");
            assert_eq!(
                canonicalize(&shim).unwrap(),
                exe_v2,
                "shim should resolve to the new 2.2.0 shim binary"
            );

            // Shims created by the new version keep using the same stable path.
            assert_eq!(homebrew_stable_bin_dir(&exe_v2), Some(stable));
        }
    }
}
