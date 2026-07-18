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

fn platform_with_pnpm(node: &str, pnpm: &str) -> String {
    format!(
        r#"{{
  "node": {{
    "runtime": "{}",
    "npm": null
  }},
  "pnpm": "{}",
  "yarn": null
}}"#,
        node, pnpm
    )
}

fn platform_with_yarn(node: &str, yarn: &str) -> String {
    format!(
        r#"{{
  "node": {{
    "runtime": "{}",
    "npm": null
  }},
  "pnpm": null,
  "yarn": "{}"
}}"#,
        node, yarn
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

// ---- npm version_source ----

#[test]
fn list_npm_default_source() {
    let s = sandbox()
        .platform(&platform_with_node_npm("10.99.1040", "6.2.26"))
        .build();

    s.create_npm_image("6.2.26");

    assert_that!(
        s.volta("list --format plain npm"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]npm@6.2.26[..]default[..]")
    );
}

#[test]
fn list_npm_project_source() {
    let s = sandbox()
        .package_json(r#"{"name":"test","volta":{"node":"10.99.1040","npm":"6.2.26"}}"#)
        .build();

    s.create_npm_image("6.2.26");

    assert_that!(
        s.volta("list --format plain npm"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]npm@6.2.26[..]current @[..]package.json[..]")
    );
}

#[test]
fn list_npm_none_source() {
    let s = sandbox()
        .platform(&platform_with_node_npm("10.99.1040", "6.2.26"))
        .build();

    s.create_npm_image("6.2.26");
    s.create_npm_image("0.0.1");

    assert_that!(
        s.volta("list --format plain npm"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("package-manager npm@0.0.1\n")
            .with_stdout_contains("[..]npm@6.2.26[..]default[..]")
    );
}

// ---- pnpm version_source ----

#[test]
fn list_pnpm_default_source() {
    let s = sandbox()
        .platform(&platform_with_pnpm("10.99.1040", "7.7.1"))
        .build();

    s.create_pnpm_image("7.7.1");

    assert_that!(
        s.volta("list --format plain pnpm"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]pnpm@7.7.1[..]default[..]")
    );
}

#[test]
fn list_pnpm_project_source() {
    let s = sandbox()
        .package_json(r#"{"name":"test","volta":{"node":"10.99.1040","pnpm":"7.7.1"}}"#)
        .build();

    s.create_pnpm_image("7.7.1");

    assert_that!(
        s.volta("list --format plain pnpm"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]pnpm@7.7.1[..]current @[..]package.json[..]")
    );
}

#[test]
fn list_pnpm_none_source() {
    let s = sandbox()
        .platform(&platform_with_pnpm("10.99.1040", "7.7.1"))
        .build();

    s.create_pnpm_image("7.7.1");
    s.create_pnpm_image("0.0.1");

    assert_that!(
        s.volta("list --format plain pnpm"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("package-manager pnpm@0.0.1\n")
            .with_stdout_contains("[..]pnpm@7.7.1[..]default[..]")
    );
}

// ---- yarn version_source ----

#[test]
fn list_yarn_default_source() {
    let s = sandbox()
        .platform(&platform_with_yarn("10.99.1040", "1.2.42"))
        .build();

    s.create_yarn_image("1.2.42");

    assert_that!(
        s.volta("list --format plain yarn"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]yarn@1.2.42[..]default[..]")
    );
}

#[test]
fn list_yarn_project_source() {
    let s = sandbox()
        .package_json(r#"{"name":"test","volta":{"node":"10.99.1040","yarn":"1.2.42"}}"#)
        .build();

    s.create_yarn_image("1.2.42");

    assert_that!(
        s.volta("list --format plain yarn"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]yarn@1.2.42[..]current @[..]package.json[..]")
    );
}

#[test]
fn list_yarn_none_source() {
    let s = sandbox()
        .platform(&platform_with_yarn("10.99.1040", "1.2.42"))
        .build();

    s.create_yarn_image("1.2.42");
    s.create_yarn_image("0.0.1");

    assert_that!(
        s.volta("list --format plain yarn"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("package-manager yarn@0.0.1\n")
            .with_stdout_contains("[..]yarn@1.2.42[..]default[..]")
    );
}

#[test]
#[cfg(unix)]
fn read_dir_error() {
    use std::os::unix::fs::PermissionsExt;

    let s = sandbox()
        .platform(&platform_with_node("10.99.1040"))
        .build();

    // Create the node image root dir then make it unreadable
    let volta_home = s.root().parent().unwrap().join("home/.volta");
    let image_dir = volta_home.join("tools/image/node");
    std::fs::create_dir_all(&image_dir).unwrap();
    std::fs::set_permissions(&image_dir, std::fs::Permissions::from_mode(0o000)).unwrap();

    // exec_with_output returns Err on non-zero exit; extract output from
    // either case to avoid panicking before restoring permissions
    let output = match s.volta("list all").exec_with_output() {
        Ok(output) => output,
        Err(err) => err.output.expect("ProcessError should contain output"),
    };

    // Restore permissions so sandbox cleanup can remove the directory
    std::fs::set_permissions(&image_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_ne!(output.status.code(), Some(ExitCode::Success as i32));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Could not read contents from directory"),
        "Expected ReadDirError in stderr, got: {}",
        stderr
    );
}
