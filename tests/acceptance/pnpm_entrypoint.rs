use std::fs;
use std::path::PathBuf;

use cfg_if::cfg_if;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use testkit::matchers::execs;
use testkit::sandbox::{DistroMetadata, PnpmFixture, Tool, sandbox};

use volta_core::constant;
use volta_core::error::ExitCode;

fn node_bin(version: &str) -> String {
    cfg_if! {
            if #[cfg(target_os = "windows")] {
                format!(
                    r#"@echo off
echo Node version {}
echo node args: %*
"#,
    version
                )
            } else {
                format!(
                    r#"#!/bin/sh
echo "Node version {}"
echo "node args: $@"
"#,
    version
                )
            }
        }
}

fn write_fake_node(s: &testkit::sandbox::Sandbox) {
    let image = s.tool(Tool::Node).version("9.27.6").image();
    cfg_if! {
        if #[cfg(windows)] {
            let path = image.join("node.bat");
        } else {
            let path = image.join("bin").join("node");
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("could not create {}: {}", parent.display(), e));
    }
    fs::write(&path, node_bin("9.27.6"))
        .unwrap_or_else(|e| panic!("could not write fake node at {}: {}", path.display(), e));
    #[cfg(unix)]
    volta_core::fs::set_executable(&path)
        .unwrap_or_else(|e| panic!("could not make {} executable: {}", path.display(), e));
}

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

const PNPM_VERSION_INFO: &str = r#"
{
    "name":"pnpm",
    "dist-tags": { "latest":"7.7.1" },
    "versions": {
        "0.0.2": { "version":"0.0.2", "dist": { "shasum":"", "tarball":"" }},
        "7.7.1": { "version":"7.7.1", "dist": { "shasum":"", "tarball":"" }},
        "12.4.2": { "version":"12.4.2", "dist": { "shasum":"", "tarball":"" }}
    }
}
"#;

const PNPM_VERSION_FIXTURES: [DistroMetadata; 3] = [
    DistroMetadata {
        version: "0.0.2",
        compressed_size: 187,
        uncompressed_size: Some(0x0028_0000),
    },
    DistroMetadata {
        version: "7.7.1",
        compressed_size: 518,
        uncompressed_size: Some(0x0028_0000),
    },
    DistroMetadata {
        version: "12.4.2",
        compressed_size: 503,
        uncompressed_size: Some(0x0028_0000),
    },
];

fn launcher_contents(s: &testkit::sandbox::Sandbox, version: &str, tool: &str) -> String {
    cfg_if! {
        if #[cfg(windows)] {
            let name = format!("{}.cmd", tool);
        } else {
            let name = tool.to_string();
        }
    }
    let path = s
        .tool(Tool::Pnpm)
        .version(version)
        .image()
        .join("bin")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read launcher at {}: {}", path.display(), e))
}

#[test]
fn pnpm_12_image_launcher_targets_mjs_entrypoint() {
    let s = sandbox()
        .layout_file("v4")
        .pnpm_available_versions(PNPM_VERSION_INFO)
        .distro_mocks::<PnpmFixture>(&PNPM_VERSION_FIXTURES)
        .env(constant::ENVNAME_VOLTA_FEATURE_PNPM, "1")
        .add_dir_to_path(PathBuf::from("/bin"))
        .add_dir_to_path(PathBuf::from("/usr/bin"))
        .build();
    s.tool(Tool::Node).version("9.27.6").image().create();
    write_fake_node(&s);

    assert_that!(
        s.volta("run --node 9.27.6 --pnpm 12.4.2 pnpm --version"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]pnpm.mjs[..] --version")
    );

    let pnpm_launcher = launcher_contents(&s, "12.4.2", "pnpm");
    assert!(
        pnpm_launcher.contains("pnpm.mjs"),
        "pnpm launcher should target pnpm.mjs:\n{}",
        pnpm_launcher
    );
    assert!(
        !pnpm_launcher.contains("pnpm.cjs"),
        "pnpm launcher should not reference pnpm.cjs:\n{}",
        pnpm_launcher
    );

    let pnpx_launcher = launcher_contents(&s, "12.4.2", "pnpx");
    assert!(
        pnpx_launcher.contains("pnpx.mjs"),
        "pnpx launcher should target pnpx.mjs:\n{}",
        pnpx_launcher
    );
    assert!(
        !pnpx_launcher.contains("pnpx.cjs"),
        "pnpx launcher should not reference pnpx.cjs:\n{}",
        pnpx_launcher
    );
}

#[test]
fn pnpm_11_image_launcher_targets_cjs_entrypoint() {
    let s = sandbox()
        .layout_file("v4")
        .pnpm_available_versions(PNPM_VERSION_INFO)
        .distro_mocks::<PnpmFixture>(&PNPM_VERSION_FIXTURES)
        .env(constant::ENVNAME_VOLTA_FEATURE_PNPM, "1")
        .add_dir_to_path(PathBuf::from("/bin"))
        .add_dir_to_path(PathBuf::from("/usr/bin"))
        .build();
    s.tool(Tool::Node).version("9.27.6").image().create();
    write_fake_node(&s);

    assert_that!(
        s.volta("run --node 9.27.6 --pnpm 7.7.1 pnpm --version"),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("[..]pnpm.cjs[..] --version")
    );

    let pnpm_launcher = launcher_contents(&s, "7.7.1", "pnpm");
    assert!(
        pnpm_launcher.contains("pnpm.cjs"),
        "pnpm launcher should target pnpm.cjs:\n{}",
        pnpm_launcher
    );
}

#[test]
fn install_pnpm_distro_without_entrypoint_errors() {
    let s = sandbox()
        .layout_file("v4")
        .platform(&platform_with_node("9.27.6"))
        .pnpm_available_versions(PNPM_VERSION_INFO)
        .distro_mocks::<PnpmFixture>(&PNPM_VERSION_FIXTURES)
        .env(constant::ENVNAME_VOLTA_FEATURE_PNPM, "1")
        .build();

    assert_that!(
        s.volta("install pnpm@0.0.2"),
        execs()
            .with_status(ExitCode::UnknownError as i32)
            .with_stderr_contains("[..]includes no supported entrypoint[..]")
    );
    assert!(
        !s.tool(Tool::Pnpm).version("0.0.2").image().exists(),
        "no image should be published for a distro without a supported entrypoint"
    );
}
