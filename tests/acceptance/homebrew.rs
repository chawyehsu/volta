//! Tests for Volta installed in a Homebrew-style layout.
//!
//! Homebrew installs Volta under a versioned Cellar directory
//! (`<prefix>/Cellar/volta/<version>/bin`) and maintains a version-stable
//! `opt` symlink (`<prefix>/opt/volta`). Shims must be created through the
//! `opt` path so they keep working when `brew upgrade` removes the old
//! Cellar version (volta-cli/volta#927).

use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use testkit::matchers::execs;
use testkit::sandbox::{cargo_dir, sandbox};

use volta_core::constant;
use volta_core::error::ExitCode;

const VERSION: &str = "2.1.0";
const UPGRADED_VERSION: &str = "2.2.0";
const DEFAULT_SHIMS: [&str; 6] = ["node", "npm", "npx", "pnpm", "yarn", "yarnpkg"];

/// Copy the built Volta binaries into a fake Homebrew Cellar layout for the
/// given version and point Homebrew's stable `opt` symlink at it, mirroring
/// what Homebrew does on install and upgrade.
fn install_volta_version(prefix: &Path, version: &str) {
    let bin = prefix.join("Cellar/volta").join(version).join("bin");
    fs::create_dir_all(&bin).expect("could not create Cellar bin dir");
    for exe in ["volta", "volta-shim", "volta-migrate"] {
        let name = format!("{exe}{}", env::consts::EXE_SUFFIX);
        fs::copy(cargo_dir().join(&name), bin.join(&name))
            .expect("could not copy binary into Cellar");
    }

    let link = prefix.join("opt/volta");
    fs::create_dir_all(link.parent().unwrap()).expect("could not create opt dir");
    fs::remove_file(&link).ok();
    symlink(format!("../Cellar/volta/{version}"), link)
        .expect("could not create Homebrew opt symlink");
}

#[test]
fn setup_links_shims_through_opt_dir_and_survives_brew_upgrade() {
    let s = sandbox().build();

    // Stage the Homebrew layout outside the sandbox's project directory.
    let prefix = s.home().parent().unwrap().join("brew");
    install_volta_version(&prefix, VERSION);

    // The stable bin directory as Volta itself resolves it (through the
    // canonicalized prefix), i.e. the target shims must be linked to.
    let canonical_prefix = fs::canonicalize(&prefix).unwrap();
    let stable_bin = canonical_prefix.join("opt/volta/bin");
    let expected_target = stable_bin.join(format!("volta-shim{}", env::consts::EXE_SUFFIX));

    // Run `volta setup` through the Homebrew `opt` path. Remove the sandbox's
    // default VOLTA_INSTALL_DIR override so that the install directory is
    // resolved from the running executable's location, as it would be for a
    // real Homebrew invocation.
    let mut p = s.process(stable_bin.join(format!("volta{}", env::consts::EXE_SUFFIX)));
    p.arg("setup");
    p.env_remove(constant::ENVNAME_VOLTA_INSTALL_DIR);
    assert_that!(p, execs().with_status(ExitCode::Success as i32));

    // Every default shim is a symlink to the stable opt path, not the
    // versioned Cellar path.
    for name in DEFAULT_SHIMS {
        let shim = s.volta_bin_dir().join(name);
        assert_eq!(
            fs::read_link(&shim).unwrap(),
            expected_target,
            "shim `{name}` should link through the stable opt dir"
        );
    }

    // Simulate `brew upgrade`: a new Cellar version is installed, Homebrew
    // retargets the `opt` symlink, and the old version directory is removed.
    // The shim symlinks themselves are left untouched.
    install_volta_version(&prefix, UPGRADED_VERSION);
    fs::remove_dir_all(prefix.join("Cellar/volta").join(VERSION))
        .expect("could not remove upgraded-away Cellar version");

    // The existing shims still resolve, now to the new version's binary.
    for name in DEFAULT_SHIMS {
        let shim = s.volta_bin_dir().join(name);
        assert_eq!(
            fs::read_link(&shim).unwrap(),
            expected_target,
            "shim `{name}` should be unchanged by the upgrade"
        );
        assert!(
            shim.exists(),
            "shim `{name}` symlink broken after simulated brew upgrade"
        );
    }

    // A shim actually executes the (new) volta-shim binary. With no toolchain
    // installed in the sandbox, running the `node` shim reports the standard
    // tool-not-installed error, which proves the shim resolved and ran. (The
    // shim binary exits with ExecutionFailure for any Volta error.)
    let shim = s.volta_bin_dir().join("node");
    let mut p = s.process(&shim);
    p.arg("--version");
    assert_that!(
        p,
        execs()
            .with_status(ExitCode::ExecutionFailure as i32)
            .with_stderr_contains("[..]Could not find executable[..]")
    );

    // Shims created after the upgrade link through the same stable path.
    let mut p = s.process(stable_bin.join(format!("volta{}", env::consts::EXE_SUFFIX)));
    p.arg("setup");
    p.env_remove(constant::ENVNAME_VOLTA_INSTALL_DIR);
    assert_that!(p, execs().with_status(ExitCode::Success as i32));

    for name in DEFAULT_SHIMS {
        let shim = s.volta_bin_dir().join(name);
        assert_eq!(
            fs::read_link(&shim).unwrap(),
            expected_target,
            "shim `{name}` created after the upgrade should link through the stable opt dir"
        );
    }
}

/// Without a Homebrew layout (no `Cellar` in the executable's path), shims
/// keep pointing at the canonicalized executable directory, as before.
#[test]
fn setup_outside_brew_links_shims_to_install_dir() {
    let s = sandbox().build();

    // The sandbox stages the binaries in the cargo target dir; there is no
    // Cellar directory in that path, so no Homebrew mapping applies.
    let mut p = s.volta("setup");
    p.env_remove(constant::ENVNAME_VOLTA_INSTALL_DIR);
    assert_that!(p, execs().with_status(ExitCode::Success as i32));

    let cargo_dir = fs::canonicalize(cargo_dir()).unwrap();
    let expected_target = cargo_dir.join(format!("volta-shim{}", env::consts::EXE_SUFFIX));
    for name in DEFAULT_SHIMS {
        let shim = s.volta_bin_dir().join(name);
        assert_eq!(
            fs::read_link(&shim).unwrap(),
            expected_target,
            "shim `{name}` should link to the install dir outside Homebrew"
        );
    }
}
