use crate::support::sandbox::sandbox;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

use volta_core::error::ExitCode;

fn platform_with_node(node: &str) -> String {
    format!(
        r#"{{
  "node": {{
    "runtime": "{}",
    "npm": null
  }},
  "pnpm": null,
  "yarn": null
}}"#,
        node
    )
}

#[cfg(windows)]
const CUSTOM_BIN: &str = "custom-bin.cmd";
#[cfg(not(windows))]
const CUSTOM_BIN: &str = "custom-bin";

#[test]
fn which_finds_binary_in_system_path() {
    let s = sandbox().executable_file(CUSTOM_BIN, "echo hello").build();
    let cmd = format!("which {}", CUSTOM_BIN);

    assert_that!(
        s.volta(&cmd),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]custom-bin[..]")
    );
}

#[test]
fn which_returns_unknown_error_when_binary_missing() {
    let s = sandbox().build();

    assert_that!(
        s.volta("which this-binary-does-not-exist"),
        execs().with_status(ExitCode::UnknownError as i32)
    );
}

#[test]
fn which_finds_default_node_binary() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .setup_node_binary("10.99.1040", "6.2.26", "echo hello")
        .build();

    assert_that!(
        s.volta("which node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node[..]")
    );
}
