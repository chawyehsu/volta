use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use node_semver::Version;
use tempfile::TempDir;

use crate::fs::PathExt;
use crate::process::ProcessBuilder;
use crate::sandbox::{EnvVar, FileBuilder};
use volta_core::constant;
use volta_core::fs::symlink_file;
use volta_core::tool::{Node, Pnpm};
use volta_layout::v4::VoltaHome;

#[must_use]
pub struct TempProjectBuilder {
    root: TempProject,
    files: Vec<FileBuilder>,
}

impl TempProjectBuilder {
    /// Root of the project, e.g. `/tmp/<tempdir>/`
    pub fn root(&self) -> PathBuf {
        self.root.root()
    }

    pub fn new() -> TempProjectBuilder {
        let tempdir = tempfile::tempdir()
            .unwrap_or_else(|e| panic!("could not create temp directory: {}", e));
        TempProjectBuilder {
            root: TempProject {
                tempdir,
                path: OsString::new(),
                env_vars: vec![],
            },
            files: vec![],
        }
    }

    /// Set the package.json for the temporary project (chainable)
    pub fn package_json(mut self, contents: &str) -> Self {
        let package_file = package_json_file(self.root());
        self.files.push(FileBuilder::new(package_file, contents));
        self
    }

    /// Create a file in the project directory (chainable)
    pub fn project_file(mut self, path: &str, contents: &str) -> Self {
        let path = self.root().join(path);
        self.files.push(FileBuilder::new(path, contents));
        self
    }

    /// Create a file in the `volta_home` directory (chainable)
    pub fn volta_home_file(mut self, path: &str, contents: &str) -> Self {
        let path = volta_home(&self.root()).join(path);
        self.files.push(FileBuilder::new(path, contents));
        self
    }

    /// Set an environment variable (chainable)
    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.root.env_vars.push(EnvVar::new(name, value));
        self
    }

    /// Create the project
    pub fn build(mut self) -> TempProject {
        let layout = VoltaHome::new(volta_home(&self.root()));

        // make sure these directories exist and are empty
        layout.node_cache_dir().ensure_empty();
        layout.shim_dir().ensure_empty();
        layout.node_inventory_dir().ensure_empty();
        layout.pnpm_inventory_dir().ensure_empty();
        layout.yarn_inventory_dir().ensure_empty();
        package_inventory_dir(&self.root()).ensure_empty();
        layout.node_image_root_dir().ensure_empty();
        layout.pnpm_image_root_dir().ensure_empty();
        layout.yarn_image_root_dir().ensure_empty();
        layout.package_image_root_dir().ensure_empty();
        layout.default_toolchain_dir().ensure_empty();
        layout.tmp_dir().ensure_empty();

        // and these files do not exist
        volta_file(&self.root()).rm();
        shim_executable(&self.root()).rm();
        layout.default_hooks_file().rm();
        layout.default_platform_file().rm();

        // create symlinks to shim executable for node, yarn, npm, and packages
        ok_or_panic!(symlink_file(shim_exe(), self.root.node_exe()));
        ok_or_panic!(symlink_file(shim_exe(), self.root.pnpm_exe()));
        ok_or_panic!(symlink_file(shim_exe(), self.root.yarn_exe()));
        ok_or_panic!(symlink_file(shim_exe(), self.root.npm_exe()));

        // write files
        for file_builder in self.files {
            file_builder.build();
        }

        // prepend Volta bin dir to the PATH
        let current_path = envoy::path().expect("Could not get current PATH");
        let new_path = current_path.split();
        self.root.path = new_path
            .prefix_entry(layout.shim_dir())
            .join()
            .expect("Failed to join paths");

        self.root
    }
}

impl Default for TempProjectBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// files and dirs in the temporary project
//
// Directory paths that mirror the current Volta layout are constructed via
// `volta_layout::v4::VoltaHome`; the helpers below are test-fixture paths
// that the layout does not (or intentionally does not) model.

fn home_dir(root: &Path) -> PathBuf {
    root.join("home")
}
fn volta_home(root: &Path) -> PathBuf {
    home_dir(root).join(".volta")
}
fn volta_file(root: &Path) -> PathBuf {
    volta_home(root).join("volta")
}
fn shim_executable(root: &Path) -> PathBuf {
    volta_home(root).join("bin").join("volta-shim")
}
fn package_inventory_dir(root: &Path) -> PathBuf {
    volta_home(root)
        .join("tools")
        .join("inventory")
        .join("packages")
}
fn package_json_file(mut root: PathBuf) -> PathBuf {
    root.push("package.json");
    root
}
fn shim_file(name: &str, root: &Path) -> PathBuf {
    volta_home(root)
        .join("bin")
        .join(format!("{}{}", name, env::consts::EXE_SUFFIX))
}
pub fn node_distro_file_name(version: &str) -> String {
    let version = Version::parse(version).unwrap();
    Node::archive_filename(&version)
}
fn npm_distro_file_name(version: &str) -> String {
    package_distro_file_name("npm", version)
}
fn pnpm_distro_file_name(version: &str) -> String {
    Pnpm::archive_filename(version)
}
fn yarn_distro_file_name(version: &str) -> String {
    format!("yarn-v{}.tar.gz", version)
}
fn package_distro_file_name(name: &str, version: &str) -> String {
    format!("{}-{}.tgz", name, version)
}

pub struct TempProject {
    tempdir: TempDir,
    path: OsString,
    env_vars: Vec<EnvVar>,
}

impl TempProject {
    /// Root of the project, e.g. `/tmp/<tempdir>/`
    pub fn root(&self) -> PathBuf {
        self.tempdir.path().to_path_buf()
    }

    /// Path of the Volta home directory inside the project home
    /// (i.e. `<root>/home/.volta`).
    pub fn volta_home(&self) -> PathBuf {
        volta_home(&self.root())
    }

    /// The Volta home layout inside the project home, used to construct
    /// paths that mirror the current Volta layout.
    fn layout(&self) -> VoltaHome {
        VoltaHome::new(self.volta_home())
    }

    /// Create a `ProcessBuilder` to run a program in the project.
    /// Example:
    ///         assert_that(
    ///             p.process(&p.bin("foo")),
    ///             execs().with_stdout("bar\n"),
    ///         );
    pub fn process<T: AsRef<OsStr>>(&self, program: T) -> ProcessBuilder {
        let mut p = crate::process::process(program);
        p.cwd(self.root())
            // setup the Volta environment
            .env("PATH", &self.path)
            .env("HOME", home_dir(&self.root()))
            .env(constant::ENVNAME_VOLTA_HOME, self.volta_home())
            .env(constant::ENVNAME_VOLTA_INSTALL_DIR, cargo_dir())
            .env_remove("VOLTA_NODE_VERSION")
            .env_remove("MSYSTEM"); // assume cmd.exe everywhere on windows

        // overrides for env vars
        for env_var in &self.env_vars {
            p.env(&env_var.name, &env_var.value);
        }

        p
    }

    /// Create a `ProcessBuilder` to run volta.
    /// Arguments can be separated by spaces.
    /// Example:
    ///     assert_that(p.volta("use node 9.5"), execs());
    pub fn volta(&self, cmd: &str) -> ProcessBuilder {
        let mut p = self.process(volta_exe());
        split_and_add_args(&mut p, cmd);
        p
    }

    /// Create a `ProcessBuilder` to run Node.
    pub fn node(&self, cmd: &str) -> ProcessBuilder {
        let mut p = self.process(self.node_exe());
        split_and_add_args(&mut p, cmd);
        p
    }

    pub fn node_exe(&self) -> PathBuf {
        self.layout()
            .shim_dir()
            .join(format!("node{}", env::consts::EXE_SUFFIX))
    }

    /// Create a `ProcessBuilder` to run Yarn.
    pub fn yarn(&self, cmd: &str) -> ProcessBuilder {
        let mut p = self.process(self.yarn_exe());
        split_and_add_args(&mut p, cmd);
        p
    }

    pub fn yarn_exe(&self) -> PathBuf {
        self.layout()
            .shim_dir()
            .join(format!("yarn{}", env::consts::EXE_SUFFIX))
    }

    /// Create a `ProcessBuilder` to run Npm.
    pub fn npm(&self, cmd: &str) -> ProcessBuilder {
        let mut p = self.process(self.npm_exe());
        split_and_add_args(&mut p, cmd);
        p
    }

    pub fn npm_exe(&self) -> PathBuf {
        self.layout()
            .shim_dir()
            .join(format!("npm{}", env::consts::EXE_SUFFIX))
    }

    /// Create a `ProcessBuilder` to run pnpm.
    pub fn pnpm(&self, cmd: &str) -> ProcessBuilder {
        let mut p = self.process(self.pnpm_exe());
        split_and_add_args(&mut p, cmd);
        p
    }

    pub fn pnpm_exe(&self) -> PathBuf {
        self.layout()
            .shim_dir()
            .join(format!("pnpm{}", env::consts::EXE_SUFFIX))
    }

    /// Create a `ProcessBuilder` to run a package executable.
    pub fn exec_shim(&self, exe: &str, cmd: &str) -> ProcessBuilder {
        let shim_file = shim_file(exe, &self.root());
        let mut p = self.process(shim_file);
        split_and_add_args(&mut p, cmd);
        p
    }

    /// Verify that the input Node version has been fetched.
    pub fn node_version_is_fetched(&self, version: &str) -> bool {
        let distro_file_name = node_distro_file_name(version);
        self.layout()
            .node_inventory_dir()
            .join(distro_file_name)
            .exists()
    }

    /// Verify that the input Node version has been unpacked.
    pub fn node_version_is_unpacked(&self, version: &str) -> bool {
        self.layout().node_image_bin_dir(version).exists()
    }

    /// Verify that the input Node version has been installed.
    pub fn assert_node_version_is_installed(&self, version: &str) {
        let platform_contents = read_file_to_string(self.layout().default_platform_file());
        let json_contents: serde_json::Value =
            serde_json::from_str(&platform_contents).expect("could not parse platform.json");
        assert_eq!(json_contents["node"]["runtime"], version);
    }

    /// Verify that the input Yarn version has been fetched.
    pub fn yarn_version_is_fetched(&self, version: &str) -> bool {
        let distro_file_name = yarn_distro_file_name(version);
        self.layout()
            .yarn_inventory_dir()
            .join(distro_file_name)
            .exists()
    }

    /// Verify that the input Yarn version has been unpacked.
    pub fn yarn_version_is_unpacked(&self, version: &str) -> bool {
        self.layout().yarn_image_dir(version).exists()
    }

    /// Verify that the input Yarn version has been installed.
    pub fn assert_yarn_version_is_installed(&self, version: &str) {
        let platform_contents = read_file_to_string(self.layout().default_platform_file());
        let json_contents: serde_json::Value =
            serde_json::from_str(&platform_contents).expect("could not parse platform.json");
        assert_eq!(json_contents["yarn"], version);
    }

    /// Verify that the input Npm version has been fetched.
    pub fn npm_version_is_fetched(&self, version: &str) -> bool {
        let distro_file_name = npm_distro_file_name(version);
        self.layout()
            .npm_inventory_dir()
            .join(distro_file_name)
            .exists()
    }

    /// Verify that the input Npm version has been unpacked.
    pub fn npm_version_is_unpacked(&self, version: &str) -> bool {
        self.layout().npm_image_bin_dir(version).exists()
    }

    /// Verify that the input Npm version has been installed.
    pub fn assert_npm_version_is_installed(&self, version: &str) {
        let platform_contents = read_file_to_string(self.layout().default_platform_file());
        let json_contents: serde_json::Value =
            serde_json::from_str(&platform_contents).expect("could not parse platform.json");
        assert_eq!(json_contents["node"]["npm"], version);
    }

    /// Verify that the input pnpm version has been fetched.
    pub fn pnpm_version_is_fetched(&self, version: &str) -> bool {
        let distro_file_name = pnpm_distro_file_name(version);
        self.layout()
            .pnpm_inventory_dir()
            .join(distro_file_name)
            .exists()
    }

    /// Verify that the input pnpm version has been unpacked.
    pub fn pnpm_version_is_unpacked(&self, version: &str) -> bool {
        self.layout().pnpm_image_bin_dir(version).exists()
    }

    /// Verify that the input pnpm version has been installed.
    pub fn assert_pnpm_version_is_installed(&self, version: &str) {
        let platform_contents = read_file_to_string(self.layout().default_platform_file());
        let json_contents: serde_json::Value =
            serde_json::from_str(&platform_contents).expect("could not parse platform.json");
        assert_eq!(json_contents["pnpm"], version);
    }

    /// Verify that the input package has been installed
    pub fn package_is_installed(&self, name: &str) -> bool {
        self.layout().package_image_dir(name).exists()
    }

    pub fn shim_exists(&self, name: &str) -> bool {
        shim_file(name, &self.root()).exists()
    }

    /// Verify that a given path in the project directory exists
    pub fn project_path_exists(&self, path: &str) -> bool {
        self.root().join(path).exists()
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        self.tempdir.path().rm_rf();
    }
}

// Generates a temporary project environment
pub fn temp_project() -> TempProjectBuilder {
    TempProjectBuilder::new()
}

// Path to compiled executables
pub fn cargo_dir() -> PathBuf {
    env::var_os("CARGO_BIN_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            env::current_exe().ok().map(|mut path| {
                path.pop();
                if path.ends_with("deps") {
                    path.pop();
                }
                path
            })
        })
        .unwrap_or_else(|| panic!("CARGO_BIN_PATH wasn't set. Cannot continue running test"))
}

fn volta_exe() -> PathBuf {
    cargo_dir().join(format!("volta{}", env::consts::EXE_SUFFIX))
}

fn shim_exe() -> PathBuf {
    cargo_dir().join(format!("volta-shim{}", env::consts::EXE_SUFFIX))
}

fn split_and_add_args(p: &mut ProcessBuilder, s: &str) {
    for arg in s.split_whitespace() {
        if arg.contains('"') || arg.contains('\'') {
            panic!("shell-style argument parsing is not supported")
        }
        p.arg(arg);
    }
}

fn read_file_to_string(file_path: &Path) -> String {
    let mut contents = String::new();
    let mut file = ok_or_panic! { File::open(file_path) };
    ok_or_panic! { file.read_to_string(&mut contents) };
    contents
}
