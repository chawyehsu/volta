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

fn platform_with_node_npm(node: &str, npm: &str) -> String {
    format!(
        r#"{{
  "node": {{
    "runtime": "{}",
    "npm": "{}"
  }},
  "pnpm": null,
  "yarn": null
}}"#,
        node, npm
    )
}

#[test]
fn list_active() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]default[..]")
    );
}

#[test]
fn list_active_with_npm() {
    let s = sandbox()
        .platform(&platform_with_node_npm("10.99.1040", "6.2.26"))
        .build();

    assert_that!(
        s.volta("list"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]")
            .with_stdout_contains("[..]npm@6.2.26[..]")
    );
}

#[test]
fn list_all() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list all"),
        execs().with_status(ExitCode::Success as i32)
    );
}

#[test]
fn list_node() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list node"),
        execs().with_status(ExitCode::Success as i32)
    );
}

#[test]
fn list_npm() {
    let s = sandbox()
        .platform(&platform_with_node_npm("10.99.1040", "6.2.26"))
        .build();

    assert_that!(
        s.volta("list npm"),
        execs().with_status(ExitCode::Success as i32)
    );
}

#[test]
fn list_pnpm() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list pnpm"),
        execs().with_status(ExitCode::Success as i32)
    );
}

#[test]
fn list_yarn() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list yarn"),
        execs().with_status(ExitCode::Success as i32)
    );
}

#[test]
fn list_package() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list cowsay"),
        execs().with_status(ExitCode::Success as i32)
    );
}

#[test]
fn list_default_filter() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list --default"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]default[..]")
    );
}

#[test]
fn list_current_filter() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list --current"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]")
    );
}

#[test]
fn list_plain_format() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    assert_that!(
        s.volta("list --format plain"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node[..]10.99.1040[..]")
    );
}

// ---- version_source tests ----

/// When the project's package.json pins a node version, `volta list node`
/// shows it as `(current @ <path>)`.
#[test]
fn list_node_project_source() {
    let s = sandbox()
        .package_json(r#"{"name":"test","volta":{"node":"10.99.1040"}}"#)
        .build();

    s.create_node_image("10.99.1040");

    assert_that!(
        s.volta("list --format plain node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]current @[..]package.json[..]")
    );
}

/// When the default platform pins a node version, `volta list node`
/// shows it as `(default)`.
#[test]
fn list_node_default_source() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    s.create_node_image("10.99.1040");

    assert_that!(
        s.volta("list --format plain node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]default[..]")
    );
}

/// A version in the inventory that is not referenced by the project or
/// the default platform has no source annotation.
#[test]
fn list_node_none_source() {
    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    s.create_node_image("10.99.1040");
    s.create_node_image("0.0.1");

    assert_that!(
        s.volta("list --format plain node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("runtime node@0.0.1\n")
            .with_stdout_contains("[..]node@10.99.1040[..]default[..]")
    );
}
