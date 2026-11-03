use crate::support::sandbox::{DistroMetadata, NpmFixture, sandbox};
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

use volta_core::error::ExitCode;

const NODE_VERSION_INFO: &str = r#"[
{"version":"v10.99.1040","npm":"6.2.26","lts": "Dubnium","files":["linux-x64","osx-x64-tar","win-x64-zip","win-x86-zip", "linux-arm64"]},
{"version":"v9.27.6","npm":"5.6.17","lts": false,"files":["linux-x64","osx-x64-tar","win-x64-zip","win-x86-zip", "linux-arm64"]},
{"version":"v8.9.10","npm":"5.6.7","lts": false,"files":["linux-x64","osx-x64-tar","win-x64-zip","win-x86-zip", "linux-arm64"]}
]
"#;

const NPM_VERSION_INFO: &str = r#"
{
    "name":"npm",
    "dist-tags": { "latest":"8.1.5" },
    "versions": {
        "1.2.3": { "version":"1.2.3", "dist": { "shasum":"", "tarball":"" }},
        "8.1.5": { "version":"8.1.5", "dist": { "shasum":"", "tarball":"" }}
    }
}
"#;

const NPM_VERSION_FIXTURES: [DistroMetadata; 2] = [
    DistroMetadata {
        version: "1.2.3",
        compressed_size: 239,
        uncompressed_size: Some(0x0028_0000),
    },
    DistroMetadata {
        version: "8.1.5",
        compressed_size: 239,
        uncompressed_size: Some(0x0028_0000),
    },
];

const BASIC_PACKAGE_JSON: &str = r#"{
  "name": "test-package"
}"#;

#[test]
fn list_node_from_standalone_node_version() {
    let s = sandbox()
        .project_file(".node-version", "10.99.1040\n")
        .build();

    s.create_node_image("10.99.1040");

    assert_that!(
        s.volta("list --format plain node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]current @[..].node-version[..]")
    );
}

#[test]
fn list_node_from_walked_up_node_version() {
    let s = sandbox()
        .package_json(BASIC_PACKAGE_JSON)
        .project_file(".node-version", "10.99.1040\n")
        .project_file("subdir/.keep", "")
        .build();

    s.create_node_image("10.99.1040");

    let mut command = s.volta("list --format plain node");
    command.cwd(s.root().join("subdir"));

    assert_that!(
        command,
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]current @[..].node-version[..]")
    );
}

#[test]
fn list_node_from_partial_node_version() {
    let s = sandbox()
        .project_file(".node-version", "10.99\n")
        .node_available_versions(NODE_VERSION_INFO)
        .build();

    s.create_node_image("10.99.1040");

    assert_that!(
        s.volta("list --format plain node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]current @[..].node-version[..]")
    );
}

#[test]
fn package_json_node_wins_over_node_version() {
    let s = sandbox()
        .package_json(r#"{"name":"test","volta":{"node":"10.99.1040"}}"#)
        .project_file(".node-version", "lts/*\n")
        .build();

    s.create_node_image("10.99.1040");

    assert_that!(
        s.volta("list --format plain node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]current @[..]package.json[..]")
    );
}

#[test]
fn malformed_node_version_errors() {
    let s = sandbox().project_file(".node-version", "lts/*\n").build();

    assert_that!(
        s.volta("list node"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]The .node-version file at[..]is malformed[..]")
    );
}

#[test]
fn pin_npm_with_node_version() {
    let s = sandbox()
        .package_json(BASIC_PACKAGE_JSON)
        .project_file(".node-version", "10.99.1040\n")
        .npm_available_versions(NPM_VERSION_INFO)
        .distro_mocks::<NpmFixture>(&NPM_VERSION_FIXTURES)
        .build();

    assert_that!(
        s.volta("pin npm@1.2.3"),
        execs().with_status(ExitCode::Success as i32)
    );

    let package_json = s.read_package_json();
    assert!(package_json.contains(r#""npm": "1.2.3""#));
    assert!(!package_json.contains(r#""node""#));
}

#[test]
fn package_managers_merge_with_node_version() {
    let s = sandbox()
        .package_json(r#"{"name":"test","volta":{"npm":"6.2.26"}}"#)
        .project_file(".node-version", "10.99.1040\n")
        .build();

    s.create_node_image("10.99.1040");
    s.create_npm_image("6.2.26");

    assert_that!(
        s.volta("list --format plain node"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]node@10.99.1040[..]current @[..].node-version[..]")
    );

    assert_that!(
        s.volta("list --format plain npm"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]npm@6.2.26[..]current @[..]package.json[..]")
    );
}

#[test]
fn unmatched_node_version_errors_with_file() {
    let s = sandbox()
        .project_file(".node-version", "99.99\n")
        .node_available_versions(NODE_VERSION_INFO)
        .build();

    assert_that!(
        s.volta("list node"),
        execs()
            .with_status(ExitCode::NoVersionMatch as i32)
            .with_stderr_contains("[..]requested by the .node-version file at[..]")
    );
}

#[test]
fn pin_node_in_standalone_node_version_errors() {
    let s = sandbox()
        .project_file(".node-version", "10.99.1040\n")
        .node_available_versions(NODE_VERSION_INFO)
        .build();

    assert_that!(
        s.volta("pin node@10.99.1040"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Not in a node package.[..]")
    );
}
