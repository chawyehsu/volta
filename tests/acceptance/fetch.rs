//! Tests for `volta fetch`.

use std::path::Path;

use crate::support::sandbox::{sandbox, DistroMetadata, NodeFixture, NpmFixture, Yarn1Fixture};
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use node_semver::Version;
use test_support::matchers::execs;
use volta_core::error::ExitCode;
use volta_core::tool::Node;

const NODE_VERSION_INFO: &str = r#"[
{"version":"v10.99.1040","npm":"6.2.26","lts": "Dubnium","files":["linux-x64","osx-x64-tar","win-x64-zip","win-x86-zip", "linux-arm64"]},
{"version":"v0.0.1","npm":"0.0.2","lts": "Sure","files":["linux-x64","osx-x64-tar","win-x64-zip","win-x86-zip", "linux-arm64"]}
]
"#;

const NODE_VERSION_FIXTURES: [DistroMetadata; 2] = [
    DistroMetadata {
        version: "0.0.1",
        compressed_size: 10,
        uncompressed_size: Some(0x0028_0000),
    },
    DistroMetadata {
        version: "10.99.1040",
        compressed_size: 273,
        uncompressed_size: Some(0x0028_0000),
    },
];

const NPM_VERSION_INFO: &str = r#"
{
    "name":"npm",
    "dist-tags": { "latest":"4.5.6" },
    "versions": {
        "4.5.6": { "version":"4.5.6", "dist": { "shasum":"", "tarball":"" }}
    }
}
"#;

const NPM_VERSION_FIXTURES: [DistroMetadata; 1] = [DistroMetadata {
    version: "4.5.6",
    compressed_size: 291,
    uncompressed_size: Some(0x0028_0000),
}];

const YARN_1_VERSION_INFO: &str = r#"{
    "name":"yarn",
    "dist-tags": { "latest": "1.2.42" },
    "versions": {
        "1.2.42": { "version":"1.2.42", "dist": { "shasum":"", "tarball":"" }}
    }
}"#;

const YARN_1_VERSION_FIXTURES: [DistroMetadata; 1] = [DistroMetadata {
    version: "1.2.42",
    compressed_size: 226,
    uncompressed_size: Some(0x0028_0000),
}];

// ---- error cases ----

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

// ---- node fetch: exercises archive::fetch_native (remote download) ----

#[test]
fn fetch_node_downloads_and_saves_to_inventory() {
    let s = sandbox()
        .node_available_versions(NODE_VERSION_INFO)
        .distro_mocks::<NodeFixture>(&NODE_VERSION_FIXTURES)
        .build();

    assert_that!(
        s.volta("fetch node@10.99.1040"),
        execs().with_status(ExitCode::Success as i32)
    );

    assert!(s.node_inventory_archive_exists(&Version::parse("10.99.1040").unwrap()));
}

#[test]
fn fetch_corrupted_node_errors() {
    let s = sandbox()
        .node_available_versions(NODE_VERSION_INFO)
        .distro_mocks::<NodeFixture>(&NODE_VERSION_FIXTURES)
        .build();

    assert_that!(
        s.volta("fetch node@0.0.1"),
        execs().with_status(ExitCode::UnknownError as i32)
    );

    assert!(!s.node_inventory_archive_exists(&Version::parse("0.0.1").unwrap()));
}

// ---- node fetch: exercises archive::load_native (cache hit) ----

#[test]
fn fetch_node_uses_cached_archive() {
    let s = sandbox()
        .node_available_versions(NODE_VERSION_INFO)
        .distro_mocks::<NodeFixture>(&NODE_VERSION_FIXTURES)
        .build();

    // Pre-populate the inventory cache with a valid fixture so that
    // load_cached_distro finds it and archive::load_native is exercised.
    let version = Version::parse("10.99.1040").unwrap();
    let fixture = Path::new("tests/fixtures").join(Node::archive_filename(&version));
    s.populate_node_inventory_cache(&version, &fixture);

    assert_that!(
        s.volta("fetch node@10.99.1040"),
        execs().with_status(ExitCode::Success as i32)
    );

    // Archive should still be present after the fetch
    assert!(s.node_inventory_archive_exists(&version));
}

// ---- npm fetch ----

#[test]
fn fetch_npm_downloads_and_saves_to_inventory() {
    let s = sandbox()
        .npm_available_versions(NPM_VERSION_INFO)
        .distro_mocks::<NpmFixture>(&NPM_VERSION_FIXTURES)
        .build();

    assert_that!(
        s.volta("fetch npm@4.5.6"),
        execs().with_status(ExitCode::Success as i32)
    );

    assert!(s.npm_inventory_archive_exists("4.5.6"));
}

// ---- yarn fetch ----

#[test]
fn fetch_yarn_downloads_and_saves_to_inventory() {
    let s = sandbox()
        .yarn_1_available_versions(YARN_1_VERSION_INFO)
        .distro_mocks::<Yarn1Fixture>(&YARN_1_VERSION_FIXTURES)
        .build();

    assert_that!(
        s.volta("fetch yarn@1.2.42"),
        execs().with_status(ExitCode::Success as i32)
    );

    assert!(s.yarn_inventory_archive_exists("1.2.42"));
}
