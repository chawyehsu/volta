use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use cfg_if::cfg_if;
use headers::{Expires, Header};
use mockito::Matcher;
use node_semver::Version;
use tempfile::TempDir;

use crate::fs::PathExt;
use crate::process::ProcessBuilder;
use volta_core::constant;
use volta_core::fs::{set_executable, symlink_file};
use volta_core::tool::{Node, Npm, Pnpm, Yarn};

// version cache for node and yarn
#[derive(PartialEq, Clone)]
struct CacheBuilder {
    path: PathBuf,
    expiry_path: PathBuf,
    contents: String,
    expired: bool,
}

impl CacheBuilder {
    pub fn new(path: PathBuf, expiry_path: PathBuf, contents: &str, expired: bool) -> CacheBuilder {
        CacheBuilder {
            path,
            expiry_path,
            contents: contents.to_string(),
            expired,
        }
    }

    fn build(&self) {
        self.dirname().mkdir_p();

        // write cache file
        let mut cache_file = File::create(&self.path).unwrap_or_else(|e| {
            panic!("could not create cache file {}: {}", self.path.display(), e)
        });
        ok_or_panic! { cache_file.write_all(self.contents.as_bytes()) };

        // write expiry file
        let one_day = Duration::from_secs(24 * 60 * 60);
        let expiry_date = Expires::from(if self.expired {
            SystemTime::now() - one_day
        } else {
            SystemTime::now() + one_day
        });

        let mut header_values = Vec::with_capacity(1);
        expiry_date.encode(&mut header_values);
        // Since we just `.encode()`d into `header_values, it is guaranteed to
        // have a `.first()`.
        let encoded_expiry_date = header_values.first().unwrap();

        let mut expiry_file = File::create(&self.expiry_path).unwrap_or_else(|e| {
            panic!(
                "could not create cache expiry file {}: {}",
                self.expiry_path.display(),
                e
            )
        });
        ok_or_panic! { expiry_file.write_all(encoded_expiry_date.as_bytes()) };
    }

    fn dirname(&self) -> &Path {
        self.path.parent().unwrap()
    }
}

// environment variables
pub struct EnvVar {
    name: String,
    value: String,
}

impl EnvVar {
    pub fn new(name: &str, value: &str) -> Self {
        EnvVar {
            name: name.to_string(),
            value: value.to_string(),
        }
    }
}

// used to construct sandboxed files like package.json, platform.json, etc.
#[derive(PartialEq, Eq, Clone)]
pub struct FileBuilder {
    path: PathBuf,
    contents: String,
    executable: bool,
}

impl FileBuilder {
    pub fn new(path: PathBuf, contents: &str) -> FileBuilder {
        FileBuilder {
            path,
            contents: contents.to_string(),
            executable: false,
        }
    }

    pub fn make_executable(mut self) -> Self {
        self.executable = true;
        self
    }

    pub fn build(&self) {
        self.dirname().mkdir_p();

        let mut file = File::create(&self.path)
            .unwrap_or_else(|e| panic!("could not create file {}: {}", self.path.display(), e));

        ok_or_panic! { file.write_all(self.contents.as_bytes()) };
        if self.executable {
            ok_or_panic! { set_executable(&self.path) };
        }
    }

    fn dirname(&self) -> &Path {
        self.path.parent().unwrap()
    }
}

struct ShimBuilder {
    path: PathBuf,
}

impl ShimBuilder {
    fn new(path: PathBuf) -> ShimBuilder {
        ShimBuilder { path }
    }

    fn build(&self) {
        ok_or_panic! { symlink_file(shim_exe(), &self.path) };
    }
}

// used to setup executable binaries in installed packages
pub struct PackageBinInfo {
    pub name: String,
    pub contents: String,
}

#[must_use]
pub struct SandboxBuilder {
    tempdir: TempDir,
    server: mockito::ServerGuard,
    mocks: Vec<mockito::Mock>,
    files: Vec<FileBuilder>,
    caches: Vec<CacheBuilder>,
    path_dirs: Vec<PathBuf>,
    shims: Vec<ShimBuilder>,
    has_exec_path: bool,
    env_vars: Vec<EnvVar>,
    env_vars_remove: Vec<String>,
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub trait DistroFixture: From<DistroMetadata> {
    fn server_path(&self) -> String;
    fn fixture_path(&self) -> String;
    fn metadata(&self) -> &DistroMetadata;
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct DistroMetadata {
    pub version: &'static str,
    pub compressed_size: u32,
    pub uncompressed_size: Option<u32>,
}

pub struct NodeFixture {
    pub metadata: DistroMetadata,
}

pub struct NpmFixture {
    pub metadata: DistroMetadata,
}

pub struct PnpmFixture {
    pub metadata: DistroMetadata,
}

pub struct Yarn1Fixture {
    pub metadata: DistroMetadata,
}

pub struct YarnBerryFixture {
    pub metadata: DistroMetadata,
}

impl From<DistroMetadata> for NodeFixture {
    fn from(metadata: DistroMetadata) -> Self {
        Self { metadata }
    }
}

impl From<DistroMetadata> for NpmFixture {
    fn from(metadata: DistroMetadata) -> Self {
        Self { metadata }
    }
}

impl From<DistroMetadata> for PnpmFixture {
    fn from(metadata: DistroMetadata) -> Self {
        Self { metadata }
    }
}

impl From<DistroMetadata> for Yarn1Fixture {
    fn from(metadata: DistroMetadata) -> Self {
        Self { metadata }
    }
}

impl From<DistroMetadata> for YarnBerryFixture {
    fn from(metadata: DistroMetadata) -> Self {
        Self { metadata }
    }
}

impl DistroFixture for NodeFixture {
    fn server_path(&self) -> String {
        let version = Version::parse(self.metadata.version).unwrap();
        let filename = Node::archive_filename(&version);
        format!("/v{version}/{filename}")
    }

    fn fixture_path(&self) -> String {
        let version = Version::parse(self.metadata.version).unwrap();
        let filename = Node::archive_filename(&version);
        format!("tests/fixtures/{filename}")
    }

    fn metadata(&self) -> &DistroMetadata {
        &self.metadata
    }
}

impl DistroFixture for NpmFixture {
    fn server_path(&self) -> String {
        format!("/npm/-/npm-{}.tgz", self.metadata.version)
    }

    fn fixture_path(&self) -> String {
        format!("tests/fixtures/npm-{}.tgz", self.metadata.version)
    }

    fn metadata(&self) -> &DistroMetadata {
        &self.metadata
    }
}

impl DistroFixture for PnpmFixture {
    fn server_path(&self) -> String {
        format!("/pnpm/-/pnpm-{}.tgz", self.metadata.version)
    }

    fn fixture_path(&self) -> String {
        format!("tests/fixtures/pnpm-{}.tgz", self.metadata.version)
    }

    fn metadata(&self) -> &DistroMetadata {
        &self.metadata
    }
}

impl DistroFixture for Yarn1Fixture {
    fn server_path(&self) -> String {
        format!("/yarn/-/yarn-{}.tgz", self.metadata.version)
    }

    fn fixture_path(&self) -> String {
        format!("tests/fixtures/yarn-{}.tgz", self.metadata.version)
    }

    fn metadata(&self) -> &DistroMetadata {
        &self.metadata
    }
}

impl DistroFixture for YarnBerryFixture {
    fn server_path(&self) -> String {
        format!(
            "/@yarnpkg/cli-dist/-/cli-dist-{}.tgz",
            self.metadata.version
        )
    }

    fn fixture_path(&self) -> String {
        format!("tests/fixtures/cli-dist-{}.tgz", self.metadata.version)
    }

    fn metadata(&self) -> &DistroMetadata {
        &self.metadata
    }
}

impl SandboxBuilder {
    /// Path of the project directory, i.e. the working directory for the
    /// processes launched by the sandbox.
    pub fn root(&self) -> PathBuf {
        self.tempdir.path().join("project")
    }

    /// Path of the sandbox home directory, i.e. the user home as seen by the
    /// processes launched by the sandbox.
    fn home(&self) -> PathBuf {
        self.tempdir.path().join("home")
    }

    pub fn new() -> SandboxBuilder {
        let tempdir = tempfile::tempdir()
            .unwrap_or_else(|e| panic!("could not create sandbox temp directory: {}", e));
        let home = tempdir.path().join("home");
        let default_hooks_file =
            FileBuilder::new(default_hooks_file(&home), sandbox_default_hooks_json());

        SandboxBuilder {
            tempdir,
            server: mockito::Server::new(),
            mocks: vec![],
            files: vec![default_hooks_file],
            caches: vec![],
            path_dirs: vec![volta_bin_dir(&home)],
            shims: vec![
                ShimBuilder::new(shim_file(&home, "npm")),
                ShimBuilder::new(shim_file(&home, "pnpm")),
                ShimBuilder::new(shim_file(&home, "yarn")),
            ],
            has_exec_path: false,
            env_vars: vec![],
            env_vars_remove: vec![],
        }
    }

    /// Set the Node cache for the sandbox (chainable)
    pub fn node_cache(mut self, cache: &str, expired: bool) -> Self {
        let home = self.home();
        self.caches.push(CacheBuilder::new(
            node_index_file(&home),
            node_index_expiry_file(&home),
            cache,
            expired,
        ));
        self
    }

    /// Set the package.json for the sandbox (chainable)
    pub fn package_json(mut self, contents: &str) -> Self {
        let package_file = package_json_file(self.root());
        self.files.push(FileBuilder::new(package_file, contents));
        self
    }

    /// Set the platform.json for the sandbox (chainable)
    pub fn platform(mut self, contents: &str) -> Self {
        let home = self.home();
        self.files
            .push(FileBuilder::new(default_platform_file(&home), contents));
        self
    }

    /// Set the hooks.json for the sandbox
    pub fn default_hooks(mut self, contents: &str) -> Self {
        let home = self.home();
        self.files
            .push(FileBuilder::new(default_hooks_file(&home), contents));
        self
    }

    /// Set a layout version file for the sandbox (chainable)
    pub fn layout_file(mut self, version: &str) -> Self {
        let home = self.home();
        self.files
            .push(FileBuilder::new(layout_file(&home, version), ""));
        self
    }

    /// Set an environment variable for the sandbox (chainable)
    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.env_vars.push(EnvVar::new(name, value));
        self
    }

    /// Remove an environment variable for the sandbox (chainable)
    pub fn env_remove(mut self, name: &str) -> Self {
        self.env_vars_remove.push(name.to_string());
        self
    }

    /// Setup mock to return the available node versions (chainable)
    pub fn node_available_versions(mut self, body: &str) -> Self {
        let mock = self
            .server
            .mock("GET", "/node-dist/index.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();
        self.mocks.push(mock);

        self
    }

    /// Setup mock to return the available Yarn@1 versions (chainable)
    pub fn yarn_1_available_versions(mut self, body: &str) -> Self {
        let mock = self
            .server
            .mock("GET", "/yarn")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();
        self.mocks.push(mock);
        self
    }

    /// Setup mock to return the available Yarn@2+ versions (chainable)
    pub fn yarn_berry_available_versions(mut self, body: &str) -> Self {
        let mock = self
            .server
            .mock("GET", "/@yarnpkg/cli-dist")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();
        self.mocks.push(mock);
        self
    }

    /// Setup mock to return the available npm versions (chainable)
    pub fn npm_available_versions(mut self, body: &str) -> Self {
        let mock = self
            .server
            .mock("GET", "/npm")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();
        self.mocks.push(mock);

        self
    }

    /// Setup mock to return the available pnpm versions (chainable)
    pub fn pnpm_available_versions(mut self, body: &str) -> Self {
        let mock = self
            .server
            .mock("GET", "/pnpm")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();
        self.mocks.push(mock);

        self
    }

    /// Setup mock to return a 404 for any GET request
    /// Note: Mocks are matched in reverse order, so any created _after_ this will work
    ///       While those created before will not
    pub fn mock_not_found(mut self) -> Self {
        let mock = self
            .server
            .mock("GET", Matcher::Any)
            .with_status(404)
            .create();
        self.mocks.push(mock);

        self
    }

    fn distro_mock<T: DistroFixture>(mut self, fx: &T) -> Self {
        // ISSUE(#145): this should actually use a real http server instead of these mocks

        let server_path = fx.server_path();
        let fixture_path = fx.fixture_path();

        let metadata = fx.metadata();

        if let Some(uncompressed_size) = metadata.uncompressed_size {
            // This can be abstracted when https://github.com/rust-lang/rust/issues/52963 lands.
            let uncompressed_size_bytes: [u8; 4] = [
                ((uncompressed_size & 0xff00_0000) >> 24) as u8,
                ((uncompressed_size & 0x00ff_0000) >> 16) as u8,
                ((uncompressed_size & 0x0000_ff00) >> 8) as u8,
                (uncompressed_size & 0x0000_00ff) as u8,
            ];

            let range_mock = self
                .server
                .mock("GET", &server_path[..])
                .match_header("Range", Matcher::Any)
                .with_body(uncompressed_size_bytes)
                .create();
            self.mocks.push(range_mock);
        }

        let file_mock = self
            .server
            .mock("GET", &server_path[..])
            .match_header("Range", Matcher::Missing)
            .with_header("Accept-Ranges", "bytes")
            .with_body_from_file(fixture_path)
            .create();
        self.mocks.push(file_mock);

        self
    }

    pub fn distro_mocks<T: DistroFixture>(self, fixtures: &[DistroMetadata]) -> Self {
        let mut this = self;
        for fixture in fixtures {
            this = this.distro_mock::<T>(&fixture.clone().into());
        }
        this
    }

    /// Add an arbitrary file to the sandbox home (chainable)
    pub fn file(mut self, path: &str, contents: &str) -> Self {
        let home = self.home();
        let file_name = sandbox_path(&home, path);
        self.files.push(FileBuilder::new(file_name, contents));
        self
    }

    /// Add an arbitrary file to the test project within the sandbox (chainable)
    pub fn project_file(mut self, path: &str, contents: &str) -> Self {
        let file_name = self.root().join(path);
        self.files.push(FileBuilder::new(file_name, contents));
        self
    }

    /// Add an arbitrary file to the test project within the sandbox,
    /// give it executable permissions,
    /// and add its directory to the PATH
    /// (chainable)
    pub fn executable_file(mut self, path: &str, contents: &str) -> Self {
        let file_name = self.root().join("exec").join(path);
        self.files
            .push(FileBuilder::new(file_name, contents).make_executable());
        self = self.add_exec_dir_to_path();
        self
    }

    /// Add directory to the PATH (chainable)
    pub fn add_dir_to_path(mut self, dir: PathBuf) -> Self {
        self.path_dirs.push(dir);
        self
    }

    /// Add executable directory to the PATH (chainable)
    pub fn add_exec_dir_to_path(mut self) -> Self {
        if !self.has_exec_path {
            let exec_path = self.root().join("exec");
            self.path_dirs.push(exec_path);
            self.has_exec_path = true;
        }
        self
    }

    /// Prepend executable directory to the beginning of the PATH (chainable)
    ///
    /// This is useful to test binaries shadowing volta shims.
    ///
    /// Cannot be used in combination with `add_exec_dir_to_path`, and will panic if called twice.
    /// No particular reason except it's likely a programming error.
    pub fn prepend_exec_dir_to_path(mut self) -> Self {
        if self.has_exec_path {
            panic!("need to call prepend_exec_dir_to_path before anything else");
        }

        let exec_path = self.root().join("exec");
        self.path_dirs.insert(0, exec_path);
        self.has_exec_path = true;
        self
    }

    /// Set a package config file for the sandbox (chainable)
    pub fn package_config(mut self, name: &str, contents: &str) -> Self {
        let home = self.home();
        let package_cfg_file = package_config_file(&home, name);
        self.files
            .push(FileBuilder::new(package_cfg_file, contents));
        self
    }

    /// Set a bin config file for the sandbox (chainable)
    pub fn binary_config(mut self, name: &str, contents: &str) -> Self {
        let home = self.home();
        let bin_cfg_file = binary_config_file(&home, name);
        self.files.push(FileBuilder::new(bin_cfg_file, contents));
        self
    }

    /// Set a shim file for the sandbox (chainable)
    pub fn shim(mut self, name: &str) -> Self {
        let home = self.home();
        self.shims.push(ShimBuilder::new(shim_file(&home, name)));
        self
    }

    /// Set an unpackaged package for the sandbox (chainable)
    pub fn package_image(
        mut self,
        name: &str,
        version: &str,
        bins: Option<Vec<PackageBinInfo>>,
    ) -> Self {
        let home = self.home();
        let package_img_dir = package_image_dir(&home, name);
        let package_json = package_img_dir.join("package.json");
        self.files.push(FileBuilder::new(
            package_json,
            &format!(r#"{{"name":"{}","version":"{}"}}"#, name, version),
        ));
        if let Some(bin_infos) = bins {
            for bin_info in bin_infos.iter() {
                cfg_if! {
                    if #[cfg(target_os = "windows")] {
                        let bin_path = package_img_dir.join(format!("{}.cmd", &bin_info.name));
                    } else {
                        let bin_path = package_img_dir.join("bin").join(&bin_info.name);
                    }
                }
                self.files
                    .push(FileBuilder::new(bin_path, &bin_info.contents).make_executable());
            }
        }
        self
    }

    /// Write executable project binaries into node_modules/.bin/ (chainable)
    pub fn project_bins(mut self, bins: Vec<PackageBinInfo>) -> Self {
        let project_bin_dir = self.root().join("node_modules").join(".bin");
        for bin_info in bins.iter() {
            cfg_if! {
                if #[cfg(target_os = "windows")] {
                    // in Windows, binaries have an extra file with an executable extension
                    let win_bin_path = project_bin_dir.join(format!("{}.cmd", &bin_info.name));
                    self.files.push(FileBuilder::new(win_bin_path, &bin_info.contents).make_executable());
                }
            }
            // Volta on both Windows and Unix checks for the existence of the binary with no extension
            let bin_path = project_bin_dir.join(&bin_info.name);
            self.files
                .push(FileBuilder::new(bin_path, &bin_info.contents).make_executable());
        }
        self
    }

    /// Write '.pnp.cjs' file in local project to mark as Plug-n-Play (chainable)
    pub fn project_pnp(mut self) -> Self {
        let pnp_path = self.root().join(".pnp.cjs");
        self.files.push(FileBuilder::new(pnp_path, "blegh"));
        self
    }

    /// Create the sandboxed environment
    pub fn build(mut self) -> Sandbox {
        let home = self.home();

        // create the project directory (the working directory for processes)
        ok_or_panic! { fs::create_dir_all(self.root()) };

        // make sure these directories exist
        ok_or_panic! { fs::create_dir_all(volta_bin_dir(&home)) };
        ok_or_panic! { fs::create_dir_all(node_cache_dir(&home)) };
        ok_or_panic! { fs::create_dir_all(node_inventory_dir(&home)) };
        ok_or_panic! { fs::create_dir_all(npm_inventory_dir(&home)) };
        ok_or_panic! { fs::create_dir_all(package_inventory_dir(&home)) };
        ok_or_panic! { fs::create_dir_all(pnpm_inventory_dir(&home)) };
        ok_or_panic! { fs::create_dir_all(yarn_inventory_dir(&home)) };
        ok_or_panic! { fs::create_dir_all(volta_tmp_dir(&home)) };

        // write node and yarn caches
        for cache in self.caches.iter() {
            cache.build();
        }

        // write files
        let server_url = self.server.url();
        for file_builder in self.files.iter_mut() {
            let is_hooks_file = file_builder
                .path
                .file_name()
                .is_some_and(|name| name == "hooks.json");

            // if this is the hooks file, replace the [server] placeholder with
            // the actual mock server URL
            if is_hooks_file {
                file_builder.contents = file_builder.contents.replace("[server]", &server_url);
            }
        }

        for file_builder in self.files {
            file_builder.build();
        }

        // write shims
        for shim_builder in self.shims {
            shim_builder.build();
        }

        // join dirs for the path (volta bin path is already first)
        let path = env::join_paths(self.path_dirs.iter()).unwrap();

        Sandbox {
            tempdir: self.tempdir,
            server: self.server,
            mocks: self.mocks,
            env_vars: self.env_vars,
            env_vars_remove: self.env_vars_remove,
            path,
        }
    }
}

// paths inside the sandbox home directory

fn home_dir_of(tempdir: &TempDir) -> PathBuf {
    tempdir.path().join("home")
}

fn volta_home(home: &Path) -> PathBuf {
    home.join(".volta")
}
fn volta_tmp_dir(home: &Path) -> PathBuf {
    volta_home(home).join("tmp")
}
fn volta_bin_dir(home: &Path) -> PathBuf {
    volta_home(home).join("bin")
}
fn volta_log_dir(home: &Path) -> PathBuf {
    volta_home(home).join("log")
}
fn volta_postscript(home: &Path) -> PathBuf {
    volta_tmp_dir(home).join("volta_tmp_1234.sh")
}
fn volta_tools_dir(home: &Path) -> PathBuf {
    volta_home(home).join("tools")
}
fn inventory_dir(home: &Path) -> PathBuf {
    volta_tools_dir(home).join("inventory")
}
fn user_dir(home: &Path) -> PathBuf {
    volta_tools_dir(home).join("user")
}
fn image_dir(home: &Path) -> PathBuf {
    volta_tools_dir(home).join("image")
}
fn node_inventory_dir(home: &Path) -> PathBuf {
    inventory_dir(home).join("node")
}
fn npm_inventory_dir(home: &Path) -> PathBuf {
    inventory_dir(home).join("npm")
}
fn pnpm_inventory_dir(home: &Path) -> PathBuf {
    inventory_dir(home).join("pnpm")
}
fn yarn_inventory_dir(home: &Path) -> PathBuf {
    inventory_dir(home).join("yarn")
}
fn package_inventory_dir(home: &Path) -> PathBuf {
    inventory_dir(home).join("packages")
}
fn cache_dir(home: &Path) -> PathBuf {
    volta_home(home).join("cache")
}
fn node_cache_dir(home: &Path) -> PathBuf {
    cache_dir(home).join("node")
}
fn node_index_file(home: &Path) -> PathBuf {
    node_cache_dir(home).join("index.json")
}
fn node_index_expiry_file(home: &Path) -> PathBuf {
    node_cache_dir(home).join("index.json.expires")
}
fn package_json_file(mut root: PathBuf) -> PathBuf {
    root.push("package.json");
    root
}
fn package_config_file(home: &Path, name: &str) -> PathBuf {
    user_dir(home)
        .join("packages")
        .join(format!("{}.json", name))
}
fn binary_config_file(home: &Path, name: &str) -> PathBuf {
    user_dir(home).join("bins").join(format!("{}.json", name))
}
fn shim_file(home: &Path, name: &str) -> PathBuf {
    volta_bin_dir(home).join(format!("{}{}", name, env::consts::EXE_SUFFIX))
}
fn package_image_dir(home: &Path, name: &str) -> PathBuf {
    image_dir(home).join("packages").join(name)
}
fn sandbox_path(home: &Path, path: &str) -> PathBuf {
    home.join(path)
}
fn default_platform_file(home: &Path) -> PathBuf {
    user_dir(home).join("platform.json")
}
fn default_hooks_file(home: &Path) -> PathBuf {
    volta_home(home).join("hooks.json")
}
fn layout_file(home: &Path, version: &str) -> PathBuf {
    volta_home(home).join(format!("layout.{}", version))
}

// Baseline hooks for acceptance tests that do not provide an explicit
// hooks.json. Tests that need special behavior should still override this.
fn sandbox_default_hooks_json() -> &'static str {
    r#"{
    "node": {
        "index": {
            "template": "[server]/node-dist/{{filename}}"
        },
        "latest": {
            "template": "[server]/node-dist/{{filename}}"
        },
        "distro": {
            "template": "[server]/v{{version}}/{{filename}}"
        }
    },
    "npm": {
        "index": {
            "template": "[server]/{{filename}}"
        },
        "distro": {
            "template": "[server]/npm/-/{{filename}}"
        }
    },
    "pnpm": {
        "index": {
            "template": "[server]/{{filename}}"
        },
        "distro": {
            "template": "[server]/pnpm/-/{{filename}}"
        }
    }
}"#
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

pub fn shim_exe() -> PathBuf {
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

fn read_file_to_string(file_path: PathBuf) -> String {
    let mut contents = String::new();
    let mut file = ok_or_panic! { File::open(file_path) };
    ok_or_panic! { file.read_to_string(&mut contents) };
    contents
}

/// The runtime tools whose on-disk layout tests can inspect and manipulate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Node,
    Npm,
    Pnpm,
    Yarn,
}

impl Tool {
    fn dir_name(self) -> &'static str {
        match self {
            Tool::Node => "node",
            Tool::Npm => "npm",
            Tool::Pnpm => "pnpm",
            Tool::Yarn => "yarn",
        }
    }
}

pub struct Sandbox {
    tempdir: TempDir,
    server: mockito::ServerGuard,
    // holds the created mocks so they are not dropped before the sandbox
    #[allow(dead_code)]
    mocks: Vec<mockito::Mock>,
    env_vars: Vec<EnvVar>,
    env_vars_remove: Vec<String>,
    path: OsString,
}

impl Sandbox {
    /// Path of the project directory, i.e. the working directory for the
    /// processes launched by the sandbox.
    pub fn root(&self) -> PathBuf {
        self.tempdir.path().join("project")
    }

    /// Path of the sandbox home directory, i.e. the user home as seen by the
    /// processes launched by the sandbox.
    pub fn home(&self) -> PathBuf {
        home_dir_of(&self.tempdir)
    }

    /// Path of the Volta home directory inside the sandbox home
    /// (i.e. `<home>/.volta`).
    pub fn volta_home(&self) -> PathBuf {
        volta_home(&self.home())
    }

    /// Path within the sandbox home directory, e.g. `home_path(".volta/bin")`
    #[cfg(windows)]
    pub fn home_path(&self, path: &str) -> PathBuf {
        sandbox_path(&self.home(), path)
    }

    /// Access the on-disk layout of a tool inside the sandbox's Volta home.
    pub fn tool(&self, tool: Tool) -> ToolHandle<'_> {
        ToolHandle {
            sandbox: self,
            tool,
        }
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
            // sandbox the Volta environment
            .env("HOME", self.home())
            .env(constant::ENVNAME_VOLTA_HOME, self.volta_home())
            .env(constant::ENVNAME_VOLTA_INSTALL_DIR, cargo_dir())
            .env("PATH", &self.path)
            .env("VOLTA_MOCK_SERVER_URL", self.server.url())
            .env("VOLTA_POSTSCRIPT", volta_postscript(&self.home()))
            .env_remove("VOLTA_SHELL")
            .env_remove("MSYSTEM"); // assume cmd.exe everywhere on windows

        // overrides for env vars
        for env_var in &self.env_vars {
            p.env(&env_var.name, &env_var.value);
        }

        for env_var_name in &self.env_vars_remove {
            p.env_remove(env_var_name);
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

    /// Create a `ProcessBuilder` to run the volta npm shim.
    /// Arguments can be separated by spaces.
    /// Example:
    ///     assert_that(p.npm("install ember-cli"), execs());
    pub fn npm(&self, cmd: &str) -> ProcessBuilder {
        self.exec_shim("npm", cmd)
    }

    /// Create a `ProcessBuilder` to run the volta pnpm shim.
    /// Arguments can be separated by spaces.
    /// Example:
    ///     assert_that(p.pnpm("add ember-cli"), execs());
    pub fn pnpm(&self, cmd: &str) -> ProcessBuilder {
        self.exec_shim("pnpm", cmd)
    }

    /// Create a `ProcessBuilder` to run the volta yarn shim.
    /// Arguments can be separated by spaces.
    /// Example:
    ///     assert_that(p.yarn("add ember-cli"), execs());
    pub fn yarn(&self, cmd: &str) -> ProcessBuilder {
        self.exec_shim("yarn", cmd)
    }

    /// Create a `ProcessBuilder` to run an arbitrary shim.
    /// Arguments can be separated by spaces.
    /// Example:
    ///     assert_that(p.exec_shim("cowsay", "foo bar"), execs());
    pub fn exec_shim(&self, bin: &str, cmd: &str) -> ProcessBuilder {
        let mut p = self.process(shim_file(&self.home(), bin));
        split_and_add_args(&mut p, cmd);
        p
    }

    pub fn read_package_json(&self) -> String {
        let package_file = package_json_file(self.root());
        read_file_to_string(package_file)
    }

    pub fn read_log_dir(&self) -> Option<fs::ReadDir> {
        fs::read_dir(volta_log_dir(&self.home())).ok()
    }

    pub fn remove_volta_home(&self) {
        self.volta_home().rm_rf();
    }

    // check that files in the sandbox exist

    pub fn package_config_exists(&self, name: &str) -> bool {
        package_config_file(&self.home(), name).exists()
    }
    pub fn bin_config_exists(&self, name: &str) -> bool {
        binary_config_file(&self.home(), name).exists()
    }
    pub fn shim_exists(&self, name: &str) -> bool {
        shim_file(&self.home(), name).exists()
    }
    pub fn path_exists(&self, path: &str) -> bool {
        sandbox_path(&self.home(), path).exists()
    }
    pub fn package_image_exists(&self, name: &str) -> bool {
        package_image_dir(&self.home(), name)
            .join("package.json")
            .exists()
    }
    pub fn read_default_platform(&self) -> String {
        read_file_to_string(default_platform_file(&self.home()))
    }

    /// Returns a mutable reference to the underlying mock server.
    pub fn server(&mut self) -> &mut mockito::ServerGuard {
        &mut self.server
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.tempdir.path().rm_rf();
    }
}

// Generates a sandboxed environment
pub fn sandbox() -> SandboxBuilder {
    SandboxBuilder::new()
}

/// Handle to inspect and manipulate the on-disk layout of a tool inside the
/// sandbox's Volta home.
pub struct ToolHandle<'a> {
    sandbox: &'a Sandbox,
    tool: Tool,
}

impl<'a> ToolHandle<'a> {
    /// Scope this handle to a version of the tool.
    pub fn version(&self, version: &str) -> ToolVersion<'a> {
        let semver = Version::parse(version)
            .unwrap_or_else(|e| panic!("invalid tool version '{}': {}", version, e));
        ToolVersion {
            sandbox: self.sandbox,
            tool: self.tool,
            version: semver.to_string(),
            semver,
        }
    }
}

/// A tool handle scoped to a specific version, exposing the paths of its
/// cached artifacts: the unpacked image, the inventory archive, and (for
/// node) the bundled npm metadata file.
pub struct ToolVersion<'a> {
    sandbox: &'a Sandbox,
    tool: Tool,
    version: String,
    semver: Version,
}

impl ToolVersion<'_> {
    /// Path of the unpacked image directory for this version.
    pub fn image(&self) -> ImagePath {
        let home = self.sandbox.home();
        let npm_file = if self.tool == Tool::Node {
            Some(node_inventory_dir(&home).join(format!("node-v{}-npm", self.version)))
        } else {
            None
        };
        ImagePath {
            path: image_dir(&home)
                .join(self.tool.dir_name())
                .join(&self.version),
            npm_file,
        }
    }

    /// Path of the cached archive file for this version in the inventory.
    pub fn archive(&self) -> InventoryFile {
        InventoryFile(self.archive_file())
    }

    /// Path of the bundled npm metadata file for this node version.
    ///
    /// Panics for tools other than node, which have no such file.
    pub fn npm_file(&self) -> InventoryFile {
        if self.tool != Tool::Node {
            panic!("the bundled npm metadata file only exists for node images");
        }
        InventoryFile(
            node_inventory_dir(&self.sandbox.home()).join(format!("node-v{}-npm", self.version)),
        )
    }

    /// Write an executable binary for this tool version into the image's
    /// bin directory.
    pub fn binary(&self, contents: &str) {
        let bin_file = self.image().join("bin").join(format!(
            "{}{}",
            self.tool.dir_name(),
            if cfg!(windows) { ".cmd" } else { "" }
        ));
        FileBuilder::new(bin_file, contents)
            .make_executable()
            .build();
    }

    fn archive_file(&self) -> PathBuf {
        let filename = match self.tool {
            Tool::Node => Node::archive_filename(&self.semver),
            Tool::Npm => Npm::archive_filename(&self.version),
            Tool::Pnpm => Pnpm::archive_filename(&self.version),
            Tool::Yarn => Yarn::archive_filename(&self.version),
        };
        inventory_dir(&self.sandbox.home())
            .join(self.tool.dir_name())
            .join(filename)
    }
}

/// Path to an unpacked tool image inside the sandbox.
pub struct ImagePath {
    path: PathBuf,
    npm_file: Option<PathBuf>,
}

impl ImagePath {
    /// Materialize a minimal image for this version: create the image
    /// directory and, for node, the bundled npm metadata file so that
    /// session initialization can read it.
    pub fn create(&self) {
        fs::create_dir_all(&self.path).unwrap_or_else(|e| {
            panic!("could not create image dir {}: {}", self.path.display(), e)
        });

        if let Some(npm_file) = &self.npm_file {
            // preserve bundled npm metadata written before the image, e.g. by
            // an explicit `npm_file().write(..)`
            if !npm_file.exists() {
                fs::write(npm_file, "0.0.0").unwrap_or_else(|e| {
                    panic!(
                        "could not write npm version file {}: {}",
                        npm_file.display(),
                        e
                    )
                });
            }
        }
    }
}

impl Deref for ImagePath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

/// Path to a file in the sandbox inventory (a cached archive, or a tool's
/// bundled metadata file).
pub struct InventoryFile(PathBuf);

impl InventoryFile {
    /// Write the file contents, creating parent directories as needed.
    pub fn write(&self, contents: &str) {
        if let Some(parent) = self.0.parent() {
            parent.mkdir_p();
        }
        fs::write(&self.0, contents).unwrap_or_else(|e| {
            panic!("could not write inventory file {}: {}", self.0.display(), e)
        });
    }

    /// Copy a fixture file into this location, simulating a previously
    /// downloaded archive (cache hit).
    pub fn write_from(&self, fixture_path: &Path) {
        if let Some(parent) = self.0.parent() {
            parent.mkdir_p();
        }
        fs::copy(fixture_path, &self.0)
            .unwrap_or_else(|e| panic!("failed to populate inventory cache: {}", e));
    }
}

impl Deref for InventoryFile {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}
