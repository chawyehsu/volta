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
