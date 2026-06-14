//! Tests for `volta setup`.

use std::fs;

use crate::support::sandbox::sandbox;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

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
        .env("VOLTA_HOME", &volta_home.to_string_lossy())
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
        .env("VOLTA_HOME", &volta_home.to_string_lossy())
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
        .env("VOLTA_HOME", &volta_home.to_string_lossy())
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
        .env("VOLTA_HOME", &volta_home.to_string_lossy())
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
        .env("VOLTA_HOME", &volta_home.to_string_lossy())
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

/// When the shim dir is already in the user PATH on Windows,
/// `volta setup` skips the registry modification.
#[test]
#[cfg(windows)]
fn setup_skips_when_already_configured_windows() {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let builder = sandbox();
    let volta_home = builder.root().join("volta-home");
    let shim_dir = volta_home.join("bin");

    let s = builder
        .env("VOLTA_HOME", &volta_home.to_string_lossy())
        .build();

    // Prepend the shim dir to the user PATH so setup detects it's already configured
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey("Environment")
        .expect("could not open Environment key");
    let original_path: String = env.get_value("Path").expect("could not read user Path");
    let new_path = format!("{};{}", shim_dir.display(), original_path);
    env.set_value("Path", &new_path)
        .expect("could not write user Path");

    // setx is needed to propagate the change to the environment that volta reads
    std::process::Command::new("setx")
        .arg("Path")
        .arg(&new_path)
        .output()
        .expect("could not run setx");

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    // Restore original PATH
    env.set_value("Path", &original_path)
        .expect("could not restore user Path");
    std::process::Command::new("setx")
        .arg("Path")
        .arg(&original_path)
        .output()
        .expect("could not restore Path via setx");
}

/// On Windows, `volta setup` prepends the shim dir to the user PATH
/// via the registry when it is not already present.
#[test]
#[cfg(windows)]
fn setup_adds_shim_dir_to_user_path_windows() {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let builder = sandbox();
    let volta_home = builder.root().join("volta-home");
    let shim_dir = volta_home.join("bin");

    let s = builder
        .env("VOLTA_HOME", &volta_home.to_string_lossy())
        .build();

    // Save original PATH and remove the shim dir if present, so setup
    // actually performs the modification.
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey("Environment")
        .expect("could not open Environment key");
    let original_path: String = env.get_value("Path").expect("could not read user Path");
    let shim_str = shim_dir.display().to_string();

    let cleaned_path = original_path
        .split(';')
        .filter(|segment| segment.trim() != shim_str)
        .collect::<Vec<_>>()
        .join(";");

    env.set_value("Path", &cleaned_path)
        .expect("could not write cleaned Path");
    std::process::Command::new("setx")
        .arg("Path")
        .arg(&cleaned_path)
        .output()
        .expect("could not propagate cleaned Path via setx");

    assert_that!(
        s.volta("setup"),
        execs().with_status(ExitCode::Success as i32)
    );

    // Verify the shim dir was added to PATH
    let updated_path: String = env.get_value("Path").expect("could not read updated Path");
    assert!(
        updated_path.contains(&shim_str),
        "shim dir should be in user PATH after setup"
    );

    // Restore original PATH
    env.set_value("Path", &original_path)
        .expect("could not restore user Path");
    std::process::Command::new("setx")
        .arg("Path")
        .arg(&original_path)
        .output()
        .expect("could not restore Path via setx");
}
