//! Tests for `volta fetch`.

use crate::support::sandbox::sandbox;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

use volta_core::error::ExitCode;

#[test]
fn fetch_bare_version_errors() {
    let s = sandbox().build();

    assert_that!(
        s.volta("fetch 12"),
        execs()
            .with_status(ExitCode::InvalidArguments as i32)
            .with_stderr_contains("[..]error: `volta fetch 12` is not supported.")
    );
}

#[test]
fn fetch_tool_and_bare_version_errors() {
    let s = sandbox().build();

    assert_that!(
        s.volta("fetch node 12"),
        execs()
            .with_status(ExitCode::InvalidArguments as i32)
            .with_stderr_contains("[..]error: `volta fetch node 12` is not supported.")
    );
}

#[test]
fn fetch_package_errors() {
    let s = sandbox().build();

    assert_that!(
        s.volta("fetch cowsay@1.0.0"),
        execs()
            .with_status(ExitCode::InvalidArguments as i32)
            .with_stderr_contains(
                "[..]Fetching packages without installing them is not supported."
            )
    );
}
