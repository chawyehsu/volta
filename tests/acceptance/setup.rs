//! Tests for `volta setup`.

use std::fs;

use crate::support::sandbox::sandbox;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

use volta_core::constant;
use volta_core::error::ExitCode;

#[test]
#[cfg(unix)]
fn setup_reports_no_shell_profile_when_home_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let builder = sandbox();
    let home = builder.root().join("home");
    let volta_home = builder.root().join("volta-home");

    let s = builder
        .env("HOME", &home.to_string_lossy())
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .build();

    fs::create_dir_all(&home).expect("could not create sandbox home directory");
    let mut permissions = fs::metadata(&home)
        .expect("could not read sandbox home metadata")
        .permissions();
    permissions.set_mode(0o555);
    fs::set_permissions(&home, permissions).expect("could not make sandbox home read-only");

    assert_that!(
        s.volta("setup"),
        execs()
            .with_status(ExitCode::EnvironmentError as i32)
            .with_stderr_contains("[..]Could not locate user profile.")
    );
}

/// When no profile exists, `volta setup` creates `~/.profile` with
/// VOLTA_HOME and PATH exports.
#[test]
#[cfg(unix)]
fn setup_creates_profile() {
    let builder = sandbox();
    let home = builder.root().join("home");
    let volta_home = builder.root().join("volta-home");

    let s = builder
        .env("HOME", &home.to_string_lossy())
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .env("SHELL", "/bin/sh")
        .build();

    fs::create_dir_all(&home).expect("could not create home dir");
    let profile = home.join(".profile");
    assert!(!profile.exists());

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    assert!(profile.exists());
    let contents = fs::read_to_string(&profile).expect("could not read .profile");
    assert!(contents.contains("export VOLTA_HOME="));
    assert!(contents.contains("export PATH=\"$VOLTA_HOME/bin:$PATH\""));
}

/// When `~/.profile` already has content, `volta setup` appends exports
/// without discarding the original content.
#[test]
#[cfg(unix)]
fn setup_updates_existing_profile_preserving_content() {
    let builder = sandbox();
    let home = builder.root().join("home");
    let volta_home = builder.root().join("volta-home");

    let s = builder
        .env("HOME", &home.to_string_lossy())
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .env("SHELL", "/bin/sh")
        .build();

    let profile = home.join(".profile");
    fs::create_dir_all(&home).expect("could not create home dir");
    fs::write(&profile, "# my existing profile\n").expect("could not write profile");

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    let contents = fs::read_to_string(&profile).expect("could not read .profile");
    assert!(contents.contains("# my existing profile"));
    assert!(contents.contains("export VOLTA_HOME="));
}

/// When VOLTA_HOME is set and the shim dir is already in PATH,
/// `volta setup` skips profile modification.
#[test]
#[cfg(unix)]
fn setup_skips_when_already_configured() {
    let builder = sandbox();
    let home = builder.root().join("home");
    let volta_home = builder.root().join("volta-home");
    let shim_dir = volta_home.join("bin");

    // Pre-create the shim dir so regenerate_shims doesn't fail if reached
    fs::create_dir_all(&shim_dir).expect("could not create shim dir");

    let s = builder
        .env("HOME", &home.to_string_lossy())
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .env("SHELL", "/bin/sh")
        .build();

    // Prepend the shim dir to PATH so setup detects it's already configured
    let path = format!(
        "{}:{}",
        shim_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut pb = s.volta("setup");
    pb.env("PATH", &path);
    assert_that!(pb, execs().with_status(ExitCode::Success as i32));

    let profile = home.join(".profile");
    assert!(
        !profile.exists(),
        "profile should not be created when already configured"
    );
}

/// When SHELL is zsh, `volta setup` writes to `~/.zshenv`.
#[test]
#[cfg(unix)]
fn setup_creates_zshenv_for_zsh() {
    let builder = sandbox();
    let home = builder.root().join("home");
    let volta_home = builder.root().join("volta-home");

    let s = builder
        .env("HOME", &home.to_string_lossy())
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .env("SHELL", "/bin/zsh")
        .build();

    fs::create_dir_all(&home).expect("could not create home dir");
    let zshenv = home.join(".zshenv");
    assert!(!zshenv.exists());

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    assert!(zshenv.exists());
    let contents = fs::read_to_string(&zshenv).expect("could not read .zshenv");
    assert!(contents.contains("export VOLTA_HOME="));
    assert!(contents.contains("export PATH=\"$VOLTA_HOME/bin:$PATH\""));
}

/// On Windows, `volta setup` reads the user PATH from the registry and
/// prepends the shim dir via `setx` when it is not already present.
///
/// The sandbox sets VOLTA_HOME to a temp directory whose shim dir is not
/// in the real user PATH, so setup will always attempt the `setx` call.
/// We verify it succeeds and that the shim directory is created.
#[test]
#[cfg(windows)]
fn setup_modifies_user_path_windows() {
    let builder = sandbox();
    let volta_home = builder.root().join("volta-home");
    let shim_dir = volta_home.join("bin");

    let s = builder
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .build();

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    // The shim directory should have been created by regenerate_shims_for_dir
    assert!(shim_dir.exists(), "shim dir should exist after setup");
}

/// On Windows, `regenerate_shims_for_dir` (called by `volta setup`) only
/// processes `.cmd` files. Pre-existing `.cmd` shims are deleted and
/// recreated with fresh contents.
#[test]
#[cfg(windows)]
fn setup_recreates_cmd_shims_windows() {
    let builder = sandbox();
    let volta_home = builder.root().join("volta-home");
    let shim_dir = volta_home.join("bin");

    let s = builder
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .build();

    // Pre-create a .cmd shim before running setup
    fs::create_dir_all(&shim_dir).expect("could not create shim dir");
    let cowsay_cmd = shim_dir.join("cowsay.cmd");
    fs::write(&cowsay_cmd, "old contents").expect("could not write shim");

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    // The .cmd shim should have been recreated with fresh contents
    assert!(
        cowsay_cmd.exists(),
        ".cmd shim should still exist after setup"
    );
    let contents = fs::read_to_string(&cowsay_cmd).expect("could not read shim");
    assert_ne!(
        contents, "old contents",
        ".cmd shim should have new contents"
    );
}

/// On Windows, `regenerate_shims_for_dir` ignores non-`.cmd` files
/// (e.g. git bash scripts without an extension).
#[test]
#[cfg(windows)]
fn setup_ignores_non_cmd_files_in_shim_dir_windows() {
    let builder = sandbox();
    let volta_home = builder.root().join("volta-home");
    let shim_dir = volta_home.join("bin");

    let s = builder
        .env(constant::ENVNAME_VOLTA_HOME, &volta_home.to_string_lossy())
        .build();

    // Pre-create a git bash script (no extension) — should be ignored
    fs::create_dir_all(&shim_dir).expect("could not create shim dir");
    let git_bash = shim_dir.join("cowsay");
    fs::write(&git_bash, "#!/bin/bash\nvolta run cowsay \"$@\"")
        .expect("could not write git bash script");

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    // The git bash script should be untouched
    assert!(git_bash.exists(), "git bash script should still exist");
    let contents = fs::read_to_string(&git_bash).expect("could not read script");
    assert!(
        contents.starts_with("#!/bin/bash"),
        "git bash script should be unchanged"
    );
}
