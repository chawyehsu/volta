use std::path::PathBuf;

use crate::support::sandbox::{sandbox, PackageBinInfo};
use cfg_if::cfg_if;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

const PKG_CONFIG_BASIC: &str = r#"{
  "name": "cowsay",
  "version": "1.4.0",
  "platform": {
    "node": "11.10.1",
    "npm": "6.7.0",
    "yarn": null
  },
  "bins": [
    "cowsay",
    "cowthink"
  ],
  "manager": "Npm"
}"#;

const PKG_CONFIG_ENVCHECK: &str = r#"{
    "name": "envcheck",
    "version": "1.4.0",
    "platform": {
        "node": "11.10.1",
        "npm": "6.7.0",
        "yarn": null
    },
    "bins": [
        "envcheck"
    ],
    "manager": "Npm"
}"#;

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

fn npm_bin(version: &str) -> String {
    cfg_if! {
            if #[cfg(target_os = "windows")] {
                format!(
                    r#"@echo off
echo Npm version {}
echo npm args: %*
"#,
    version
                )
            } else {
                format!(
                    r#"#!/bin/sh
echo "Npm version {}"
echo "npm args: $@"
"#,
    version
                )
            }
        }
}

fn pnpm_bin(version: &str) -> String {
    cfg_if! {
            if #[cfg(target_os = "windows")] {
                format!(
                    r#"@echo off
echo pnpm version {}
echo pnpm args: %*
"#,
    version
                )
            } else {
                format!(
                    r#"#!/bin/sh
echo "pnpm version {}"
echo "pnpm args: $@"
"#,
    version
                )
            }
        }
}

fn yarn_bin(version: &str) -> String {
    cfg_if! {
            if #[cfg(target_os = "windows")] {
                format!(
                    r#"@echo off
echo Yarn version {}
echo yarn args: %*
"#,
    version
                )
            } else {
                format!(
                    r#"#!/bin/sh
echo "Yarn version {}"
echo "yarn args: $@"
"#,
    version
                )
            }
        }
}

fn cowsay_bin(name: &str, version: &str) -> String {
    cfg_if! {
        if #[cfg(target_os = "windows")] {
            format!(
                r#"@echo off
echo {} version {}
echo {} args: %*
"#,
    name, version, name
            )
        } else {
            format!(
                r#"#!/bin/sh
echo "{} version {}"
echo "{} args: $@"
"#,
    name, version, name
            )
        }
    }
}

fn envcheck_bin(name: &str, version: &str) -> String {
    cfg_if! {
        if #[cfg(target_os = "windows")] {
            format!(
                r#"@echo off
echo {} version {}
echo NODE_PATH: %NODE_PATH%
echo {} args: %*
"#,
    name, version, name
            )
        } else {
            format!(
                r#"#!/bin/sh
echo "{} version {}"
echo "NODE_PATH: $NODE_PATH"
echo "{} args: $@"
"#,
    name, version, name
            )
        }
    }
}

fn cowsay_bin_info(version: &str) -> Vec<PackageBinInfo> {
    vec![
        PackageBinInfo {
            name: "cowsay".to_string(),
            contents: cowsay_bin("cowsay", version),
        },
        PackageBinInfo {
            name: "cowthink".to_string(),
            contents: cowsay_bin("cowthink", version),
        },
    ]
}

fn envcheck_bin_info(version: &str) -> Vec<PackageBinInfo> {
    vec![PackageBinInfo {
        name: "envcheck".to_string(),
        contents: envcheck_bin("envcheck", version),
    }]
}

fn bin_config(name: &str) -> String {
    format!(
        r#"{{
  "name": "{}",
  "package": "cowsay",
  "version": "1.4.0",
  "platform": {{
    "node": "11.10.1",
    "npm": "6.7.0",
    "yarn": null
  }},
  "manager": "Npm"
}}"#,
        name
    )
}

fn envcheck_bin_config(name: &str) -> String {
    format!(
        r#"{{
    "name": "{}",
    "package": "envcheck",
    "version": "1.4.0",
    "platform": {{
        "node": "11.10.1",
        "npm": "6.7.0",
        "yarn": null
    }},
    "manager": "Npm"
}}"#,
        name
    )
}

const PACKAGE_JSON_NPM_NO_DEP: &str = r#"{
    "name": "no-deps",
    "volta": {
        "node": "10.99.1040"
    }
}"#;

const PACKAGE_JSON_NPM_WITH_DEP: &str = r#"{
    "name": "with-deps",
    "dependencies": {
        "cowsay": "1.5.0"
    },
    "volta": {
        "node": "10.99.1040"
    }
}"#;

const PACKAGE_JSON_YARN_PNP_WITH_DEP: &str = r#"{
    "name": "with-deps",
    "dependencies": {
        "cowsay": "1.5.0"
    },
    "volta": {
        "node": "10.99.1040",
        "yarn": "3.12.1092"
    }
}"#;

const PLATFORM_NODE_NPM: &str = r#"{
    "node":{
        "runtime":"11.10.1",
        "npm":"6.7.0"
    }
}"#;

#[test]
fn default_binary_no_project() {
    // platform node is 11.10.1, npm is 6.7.0
    // package cowsay is 1.4.0, installed with platform node
    // default yarn is 1.23.483
    // default pnpm is 7.7.1
    // there is no local project, so it should run the default bin
    let s = sandbox()
        .platform(PLATFORM_NODE_NPM)
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .package_image("cowsay", "1.4.0", Some(cowsay_bin_info("1.4.0")))
        .setup_node_binary("11.10.1", "6.7.0", &node_bin("11.10.1"))
        .setup_npm_binary("6.7.0", &npm_bin("6.7.0"))
        .setup_yarn_binary("1.23.483", &yarn_bin("1.23.483"))
        .setup_pnpm_binary("7.7.1", &pnpm_bin("7.7.1"))
        .add_dir_to_path(PathBuf::from("/bin"))
        .build();

    // control should be passed directly to the bin
    assert_that!(
        s.exec_shim("cowsay", "foo"),
        execs()
            .with_status(0)
            .with_stdout_contains("cowsay version 1.4.0")
            .with_stdout_contains("cowsay args: foo")
            .with_stdout_does_not_contain("Node version")
            .with_stdout_does_not_contain("Npm version")
            .with_stdout_does_not_contain("Yarn version")
            .with_stdout_does_not_contain("pnpm version")
    );
}

#[test]
fn default_binary_sets_node_path() {
    // platform node is 11.10.1, npm is 6.7.0
    // package envcheck is 1.4.0, installed with platform node
    // default yarn is 1.23.483
    // default pnpm is 7.7.1
    // there is no local project, so it should run the default bin and inject NODE_PATH
    let s = sandbox()
        .platform(PLATFORM_NODE_NPM)
        .package_config("envcheck", PKG_CONFIG_ENVCHECK)
        .binary_config("envcheck", &envcheck_bin_config("envcheck"))
        .shim("envcheck")
        .package_image("envcheck", "1.4.0", Some(envcheck_bin_info("1.4.0")))
        .setup_node_binary("11.10.1", "6.7.0", &node_bin("11.10.1"))
        .setup_npm_binary("6.7.0", &npm_bin("6.7.0"))
        .setup_yarn_binary("1.23.483", &yarn_bin("1.23.483"))
        .setup_pnpm_binary("7.7.1", &pnpm_bin("7.7.1"))
        .add_dir_to_path(PathBuf::from("/bin"))
        .build();

    assert_that!(
        s.exec_shim("envcheck", "foo"),
        execs()
            .with_status(0)
            .with_stdout_contains("envcheck version 1.4.0")
            .with_stdout_contains("NODE_PATH:[..]shared[..]")
            .with_stdout_contains("envcheck args: foo")
    );
}

#[test]
fn project_local_binary_not_found() {
    // platform node is 11.10.1, npm is 6.7.0
    // package cowsay is 1.4.0, installed with platform node
    // local project has cowsay as a dep, but no local binary exists, so it should error
    let s = sandbox()
        .package_json(PACKAGE_JSON_NPM_WITH_DEP)
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .shim("cowsay")
        .build();

    assert_that!(
        s.exec_shim("cowsay", "foo"),
        execs()
            .with_status(126)
            .with_stderr_contains("[..]Could not locate executable `cowsay` in your project.[..]")
    );
}

#[test]
fn unknown_binary_falls_back_to_default_binary_error() {
    // there is no project, and Volta does not know about this shimmed binary
    // it should fall back to the default binary path and fail to locate the executable
    let s = sandbox().shim("volta-test").build();

    cfg_if! {
        if #[cfg(target_os = "windows")] {
            assert_that!(
                s.exec_shim("volta-test", ""),
                execs()
                    .with_status(1)
                    .with_stderr_contains("'volta-test' is not recognized as an internal or external command")
            );
        } else {
            assert_that!(
                s.exec_shim("volta-test", ""),
                execs()
                    .with_status(126)
                    .with_stderr_contains("[..]Could not find executable \"volta-test\"[..]")
            );
        }
    }
}

#[test]
fn default_binary_no_project_dep() {
    // platform node is 11.10.1, npm is 6.7.0
    // package cowsay is 1.4.0, installed with platform node
    // default yarn is 1.23.483
    // default pnpm is 7.7.1
    // local project does not have cowsay dep, so it should run the default bin
    let s = sandbox()
        .platform(PLATFORM_NODE_NPM)
        .package_json(PACKAGE_JSON_NPM_NO_DEP)
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .package_image("cowsay", "1.4.0", Some(cowsay_bin_info("1.4.0")))
        .setup_node_binary("11.10.1", "6.7.0", &node_bin("11.10.1"))
        .setup_npm_binary("6.7.0", &npm_bin("6.7.0"))
        .setup_yarn_binary("1.23.483", &yarn_bin("1.23.483"))
        .setup_pnpm_binary("7.7.1", &pnpm_bin("7.7.1"))
        .add_dir_to_path(PathBuf::from("/bin"))
        .build();

    assert_that!(
        s.exec_shim("cowsay", "foo"),
        execs()
            .with_status(0)
            .with_stdout_contains("cowsay version 1.4.0")
            .with_stdout_contains("cowsay args: foo")
            .with_stdout_does_not_contain("Node version")
            .with_stdout_does_not_contain("Npm version")
            .with_stdout_does_not_contain("Yarn version")
            .with_stdout_does_not_contain("pnpm version")
    );
}

#[test]
fn project_local_binary() {
    // platform node is 11.10.1, npm is 6.7.0
    // package cowsay is 1.4.0, installed with platform node
    // default yarn is 1.23.483
    // default pnpm is 7.7.1
    // local project has cowsay as a dep, so it should run that binary
    let s = sandbox()
        .platform(PLATFORM_NODE_NPM)
        .package_json(PACKAGE_JSON_NPM_WITH_DEP)
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .package_image("cowsay", "1.4.0", Some(cowsay_bin_info("1.4.0")))
        .setup_node_binary("11.10.1", "6.7.0", &node_bin("11.10.1"))
        .setup_node_binary("10.99.1040", "6.7.0", &node_bin("10.99.1040"))
        .setup_npm_binary("6.7.0", &npm_bin("6.7.0"))
        .setup_yarn_binary("1.23.483", &yarn_bin("1.23.483"))
        .setup_pnpm_binary("7.7.1", &pnpm_bin("7.7.1"))
        .project_bins(cowsay_bin_info("1.5.0"))
        .add_dir_to_path(PathBuf::from("/bin"))
        .build();

    // control should be passed directly to the local bin
    assert_that!(
        s.exec_shim("cowsay", "bar"),
        execs()
            .with_status(0)
            .with_stdout_contains("cowsay version 1.5.0")
            .with_stdout_contains("cowsay args: bar")
            .with_stdout_does_not_contain("Node version")
            .with_stdout_does_not_contain("Npm version")
            .with_stdout_does_not_contain("Yarn version")
            .with_stdout_does_not_contain("pnpm version")
    );
}

#[test]
fn project_local_binary_pnp() {
    // platform node is 11.10.1, npm is 6.7.0
    // package cowsay is 1.4.0, installed with platform node
    // default yarn is 1.23.483
    // project is Yarn PnP, with cowsay as a dep, so it should run 'yarn cowsay'
    let s = sandbox()
        .platform(PLATFORM_NODE_NPM)
        .package_json(PACKAGE_JSON_YARN_PNP_WITH_DEP)
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .package_image("cowsay", "1.4.0", Some(cowsay_bin_info("1.4.0")))
        .setup_node_binary("11.10.1", "6.7.0", &node_bin("11.10.1"))
        .setup_node_binary("10.99.1040", "6.7.0", &node_bin("10.99.1040"))
        .setup_npm_binary("6.7.0", &npm_bin("6.7.0"))
        .setup_yarn_binary("1.23.483", &yarn_bin("1.23.483"))
        .setup_yarn_binary("3.12.1092", &yarn_bin("3.12.1092"))
        .project_pnp()
        .add_dir_to_path(PathBuf::from("/bin"))
        .build();

    // this should run 'yarn cowsay' to execute the binary
    assert_that!(
        s.exec_shim("cowsay", "baz"),
        execs()
            .with_status(0)
            .with_stdout_contains("Yarn version 3.12.1092")
            .with_stdout_contains("yarn args: cowsay baz")
            .with_stdout_does_not_contain("cowsay version")
            .with_stdout_does_not_contain("cowsay args")
            .with_stdout_does_not_contain("Node version")
            .with_stdout_does_not_contain("Npm version")
            .with_stdout_does_not_contain("Yarn version 1.23.483")
    );
}
