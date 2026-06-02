use std::path::PathBuf;
use std::{thread, time};

use crate::support::events_helpers::{
    assert_events, match_args, match_end, match_error, match_start,
};
use crate::support::sandbox::sandbox;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;
use volta_core::error::ExitCode;

const WORKSPACE_PACKAGE_JSON: &str = r#"
{
    "volta": {
        "node": "10.11.12"
    }
}"#;

const PROJECT_PACKAGE_JSON: &str = r#"
{
    "volta": {
        "extends": "./workspace/package.json"
    }
}"#;

// scripts that write events to file 'events.json'
cfg_if::cfg_if! {
    if #[cfg(windows)] {
        // have not been able to read events from stdin with batch, powershell, etc.
        // so just copy the tempfile (path in EVENTS_FILE env var) to events.json
        const EVENTS_EXECUTABLE: &str = r#"@echo off
copy %EVENTS_FILE% events.json
:: executables should clean up the temp file
del %EVENTS_FILE%
"#;
        const SCRIPT_FILENAME: &str = "write-events.bat";
        const VOLTA_BINARY: &str = "volta.exe";
    } else if #[cfg(unix)] {
        // read events from stdin
        const EVENTS_EXECUTABLE: &str = r#"#!/bin/bash
# read Volta events from stdin, and write to events.json
# (but first clear it out)
echo -n "" >events.json
while read line
do
  echo "$line" >>events.json
done
# executables should clean up the temp file
/bin/rm "$EVENTS_FILE"
"#;
        const SCRIPT_FILENAME: &str = "write-events.sh";
        const VOLTA_BINARY: &str = "volta";
    } else {
        compile_error!("Unsupported platform for tests (expected 'unix' or 'windows').");
    }
}

fn default_hooks_json() -> String {
    format!(
        r#"
{{
    "node": {{
        "distro": {{
                "template": "[server]/hook/default/node/{{{{version}}}}"
        }}
    }},
    "npm": {{
        "distro": {{
                "template": "[server]/hook/default/npm/{{{{version}}}}"
        }}
    }},
    "yarn": {{
        "distro": {{
                "template": "[server]/hook/default/yarn/{{{{version}}}}"
        }}
    }},
    "events": {{
        "publish": {{
            "bin": "{}"
        }}
    }}
}}"#,
        SCRIPT_FILENAME
    )
}

fn project_hooks_json() -> &'static str {
    r#"{
    "yarn": {
        "distro": {
            "template": "[server]/hook/project/yarn/{{version}}"
        }
    }
}"#
}

fn workspace_hooks_json() -> &'static str {
    r#"{
    "npm": {
        "distro": {
            "template": "[server]/hook/workspace/npm/{{version}}"
        }
    },
    "yarn": {
        "distro": {
            "template": "[server]/hook/workspace/yarn/{{version}}"
        }
    }
}"#
}

fn pnpm_hooks_json() -> &'static str {
    r#"{
    "pnpm": {
        "index": {
            "template": "[server]/pnpm/index"
        },
        "distro": {
            "template": "[server]/pnpm/{{version}}"
        }
    }
}"#
}

fn yarn_hooks_json() -> &'static str {
    r#"{
    "yarn": {
        "latest": {
            "template": "[server]/yarn-old/latest"
        },
        "index": {
            "template": "[server]/yarn-old/index"
        }
    }
}"#
}

fn yarn_hooks_format_json(format: &str) -> String {
    format!(
        r#"
{{
    "yarn": {{
        "latest": {{
            "template": "[server]/yarn-new/latest"
        }},
        "index": {{
            "template": "[server]/yarn-new/index",
            "format": "{0}"
        }}
    }}
}}"#,
        format
    )
}

#[test]
fn malformed_default_hooks_errors() {
    let s = sandbox().default_hooks("{").build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Could not parse hooks configuration file.")
    );
}

#[test]
fn malformed_publish_hook_with_url_and_bin_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "node": {
        "distro": {
            "template": "[server]/hook/default/node/{{version}}"
        }
    },
    "events": {
        "publish": {
            "url": "[server]/events/publish",
            "bin": "write-events.sh"
        }
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Publish hook configuration includes both hook types.")
    );
}

#[test]
fn malformed_publish_hook_without_url_or_bin_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "node": {
        "distro": {
            "template": "[server]/hook/default/node/{{version}}"
        }
    },
    "events": {
        "publish": {}
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Publish hook configuration includes no hook types.")
    );
}

#[test]
fn malformed_tool_hook_without_type_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "node": {
        "distro": {}
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Hook configuration includes no hook types.")
    );
}

#[test]
fn malformed_tool_hook_with_multiple_types_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "node": {
        "distro": {
            "prefix": "[server]/hook/default/node/",
            "template": "[server]/hook/default/node/{{version}}"
        }
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Hook configuration includes multiple hook types.")
    );
}

#[test]
fn tool_hook_with_empty_bin_command_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "node": {
        "distro": {
            "bin": "   "
        }
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ExecutableNotFound as i32)
            .with_stderr_contains("[..]Invalid hook command: ''")
    );
}

#[test]
fn tool_hook_with_missing_relative_bin_path_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "node": {
        "distro": {
            "bin": "./missing-hook-script"
        }
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains(
                "[..]Could not determine path to hook command: './missing-hook-script'"
            )
    );
}

#[test]
fn tool_hook_with_failing_bin_command_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "node": {
        "distro": {
            "bin": "node"
        }
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Hook command 'node' indicated a failure.")
    );
}

#[test]
fn redirects_download() {
    let s = sandbox()
        .default_hooks(&default_hooks_json())
        .env("VOLTA_WRITE_EVENTS_FILE", "true")
        .executable_file(SCRIPT_FILENAME, EVENTS_EXECUTABLE)
        .build();

    assert_that!(
        s.volta("install node@1.2.3"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not download node@1.2.3")
            .with_stderr_contains("[..]/hook/default/node/1.2.3")
    );

    thread::sleep(time::Duration::from_millis(500));
    assert_events(
        &s,
        vec![
            ("volta", match_start()),
            ("install", match_start()),
            ("volta", match_error(5, "Could not download node")),
            ("volta", match_end(5)),
            (
                "args",
                match_args(format!("{} install node@1.2.3", VOLTA_BINARY).as_str()),
            ),
        ],
    );
}

#[test]
fn merges_project_and_default_hooks() {
    let local_hooks: PathBuf = [".volta", "hooks.json"].iter().collect();
    let s = sandbox()
        .package_json("{}")
        .default_hooks(&default_hooks_json())
        .project_file(&local_hooks.to_string_lossy(), &project_hooks_json())
        .env("VOLTA_WRITE_EVENTS_FILE", "true")
        .executable_file(SCRIPT_FILENAME, EVENTS_EXECUTABLE)
        .build();

    // Project defines yarn hooks, so those should be used
    assert_that!(
        s.volta("install yarn@3.2.1"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not download yarn@3.2.1")
            .with_stderr_contains("[..]/hook/project/yarn/3.2.1")
    );
    thread::sleep(time::Duration::from_millis(500));
    assert_events(
        &s,
        vec![
            ("volta", match_start()),
            ("install", match_start()),
            ("volta", match_error(5, "Could not download yarn")),
            ("volta", match_end(5)),
            (
                "args",
                match_args(format!("{} install yarn@3.2.1", VOLTA_BINARY).as_str()),
            ),
        ],
    );

    // Project doesn't define node hooks, so should inherit from the default
    assert_that!(
        s.volta("install node@10.12.1"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not download node@10.12.1")
            .with_stderr_contains("[..]/hook/default/node/10.12.1")
    );
    thread::sleep(time::Duration::from_millis(500));
    assert_events(
        &s,
        vec![
            ("volta", match_start()),
            ("install", match_start()),
            ("volta", match_error(5, "Could not download node")),
            ("volta", match_end(5)),
            (
                "args",
                match_args(format!("{} install node@10.12.1", VOLTA_BINARY).as_str()),
            ),
        ],
    );
}

#[test]
fn merges_workspace_hooks() {
    let workspace_hooks: PathBuf = ["workspace", ".volta", "hooks.json"].iter().collect();
    let workspace_package_json: PathBuf = ["workspace", "package.json"].iter().collect();
    let project_hooks: PathBuf = [".volta", "hooks.json"].iter().collect();
    let s = sandbox()
        .default_hooks(&default_hooks_json())
        .package_json(PROJECT_PACKAGE_JSON)
        .project_file(&project_hooks.to_string_lossy(), &project_hooks_json())
        .project_file(
            &workspace_package_json.to_string_lossy(),
            WORKSPACE_PACKAGE_JSON,
        )
        .project_file(&workspace_hooks.to_string_lossy(), &workspace_hooks_json())
        .env("VOLTA_WRITE_EVENTS_FILE", "true")
        .executable_file(SCRIPT_FILENAME, EVENTS_EXECUTABLE)
        .build();

    // Project defines yarn hooks, so those should be used
    assert_that!(
        s.volta("pin yarn@3.1.4"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not download yarn@3.1.4")
            .with_stderr_contains("[..]/hook/project/yarn/3.1.4")
    );
    thread::sleep(time::Duration::from_millis(500));
    assert_events(
        &s,
        vec![
            ("volta", match_start()),
            ("pin", match_start()),
            ("volta", match_error(5, "Could not download yarn")),
            ("volta", match_end(5)),
            (
                "args",
                match_args(format!("{} pin yarn@3.1.4", VOLTA_BINARY).as_str()),
            ),
        ],
    );

    // Workspace defines npm hooks, so those should be inherited
    assert_that!(
        s.volta("pin npm@5.6.7"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not download npm@5.6.7")
            .with_stderr_contains("[..]/hook/workspace/npm/5.6.7")
    );
    thread::sleep(time::Duration::from_millis(500));
    assert_events(
        &s,
        vec![
            ("volta", match_start()),
            ("pin", match_start()),
            ("volta", match_error(5, "Could not download npm")),
            ("volta", match_end(5)),
            (
                "args",
                match_args(format!("{} pin npm@5.6.7", VOLTA_BINARY).as_str()),
            ),
        ],
    );

    // Neither project nor workspace defines node hooks, so should inherit from the default
    assert_that!(
        s.volta("install node@11.11.2"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not download node@11.11.2")
            .with_stderr_contains("[..]/hook/default/node/11.11.2")
    );
}

#[test]
fn pnpm_latest_with_hook_reads_index() {
    let mut s = sandbox()
        .default_hooks(&pnpm_hooks_json())
        .env("VOLTA_LOGLEVEL", "debug")
        .env("VOLTA_FEATURE_PNPM", "1")
        .build();
    let _mock = s
        .server()
        .mock("GET", "/pnpm/index")
        .with_status(200)
        .with_header("Content-Type", "application/json")
        .with_body(
            // Npm format for pnpm
            r#"{
    "name":"pnpm",
    "dist-tags": { "latest":"7.7.1" },
    "versions": {
        "0.0.1": { "version":"0.0.1", "dist": { "shasum":"", "tarball":"" }},
        "6.34.0": { "version":"6.34.0", "dist": { "shasum":"", "tarball":"" }},
        "7.7.1": { "version":"7.7.1", "dist": { "shasum":"", "tarball":"" }}
    }
}"#,
        )
        .create();

    assert_that!(
        s.volta("install pnpm@latest"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Using pnpm.index hook to determine pnpm index URL")
            .with_stderr_contains("[..]Found pnpm@7.7.1 matching tag 'latest'[..]")
            .with_stderr_contains("[..]Downloading pnpm@7.7.1 from[..]/pnpm/7.7.1[..]")
            .with_stderr_contains("[..]Could not download pnpm@7.7.1")
    );
}

#[test]
fn pnpm_no_version_with_hook_reads_index() {
    let mut s = sandbox()
        .default_hooks(&pnpm_hooks_json())
        .env("VOLTA_LOGLEVEL", "debug")
        .env("VOLTA_FEATURE_PNPM", "1")
        .build();
    let _mock = s
        .server()
        .mock("GET", "/pnpm/index")
        .with_status(200)
        .with_header("Content-Type", "application/json")
        .with_body(
            // Npm format for pnpm
            r#"{
    "name":"pnpm",
    "dist-tags": { "latest":"7.7.1" },
    "versions": {
        "0.0.1": { "version":"0.0.1", "dist": { "shasum":"", "tarball":"" }},
        "6.34.0": { "version":"6.34.0", "dist": { "shasum":"", "tarball":"" }},
        "7.7.1": { "version":"7.7.1", "dist": { "shasum":"", "tarball":"" }}
    }
}"#,
        )
        .create();

    assert_that!(
        s.volta("install pnpm"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Using pnpm.index hook to determine pnpm index URL")
            .with_stderr_contains("[..]Found pnpm@7.7.1 matching tag 'latest'[..]")
            .with_stderr_contains("[..]Downloading pnpm@7.7.1 from[..]/pnpm/7.7.1[..]")
            .with_stderr_contains("[..]Could not download pnpm@7.7.1")
    );
}

#[test]
fn yarn_latest_with_hook_reads_latest() {
    let mut s = sandbox()
        .default_hooks(&yarn_hooks_json())
        .env("VOLTA_LOGLEVEL", "debug")
        .build();
    let _mock = s
        .server()
        .mock("GET", "/yarn-old/latest")
        .with_status(200)
        .with_body("4.2.9")
        .create();

    assert_that!(
        s.volta("install yarn@latest"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Using yarn.latest hook to determine latest-version URL")
            .with_stderr_contains("[..]Found yarn latest version (4.2.9)[..]")
            .with_stderr_contains("[..]Could not download yarn@4.2.9")
    );
}

#[test]
fn yarn_no_version_with_hook_reads_latest() {
    let mut s = sandbox()
        .default_hooks(&yarn_hooks_json())
        .env("VOLTA_LOGLEVEL", "debug")
        .build();
    let _mock = s
        .server()
        .mock("GET", "/yarn-old/latest")
        .with_status(200)
        .with_body("4.2.9")
        .create();

    assert_that!(
        s.volta("install yarn"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Using yarn.latest hook to determine latest-version URL")
            .with_stderr_contains("[..]Found yarn latest version (4.2.9)[..]")
            .with_stderr_contains("[..]Could not download yarn@4.2.9")
    );
}

#[test]
fn yarn_semver_with_hook_uses_old_format() {
    let mut s = sandbox()
        .default_hooks(&yarn_hooks_json())
        .env("VOLTA_LOGLEVEL", "debug")
        .build();
    let _mock = s
        .server()
        .mock("GET", "/yarn-old/index")
        .with_status(200)
        .with_header("Content-Type", "application/json")
        .with_body(
            // Yarn Index hook expects the "old" (Github API) format
            r#"[
    {"tag_name":"v1.22.4","assets":[{"name":"yarn-v1.22.4.tar.gz"}]},
    {"tag_name":"v2.0.0","assets":[{"name":"yarn-v2.0.0.tar.gz"}]},
    {"tag_name":"v3.9.2","assets":[{"name":"yarn-v3.9.2.tar.gz"}]},
    {"tag_name":"v4.1.1","assets":[{"name":"yarn-v4.1.1.tar.gz"}]}
]"#,
        )
        .create();

    assert_that!(
        s.volta("install yarn@3"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Using yarn.index hook to determine yarn index URL")
            .with_stderr_contains("[..]Found yarn@3.9.2 matching requirement[..]")
            .with_stderr_contains("[..]Could not download yarn@3.9.2")
    );
}

#[test]
fn yarn_semver_with_hook_uses_configured_format() {
    let mut s = sandbox()
        .default_hooks(&yarn_hooks_format_json("npm"))
        .env("VOLTA_LOGLEVEL", "debug")
        .build();
    let _mock = s
        .server()
        .mock("GET", "/yarn-new/index")
        .with_status(200)
        .with_header("Content-Type", "application/json")
        .with_body(
            // Should be using the Npm format
            r#"{
    "name":"@yarnpkg/cli-dist",
    "dist-tags": { "latest":"3.12.99" },
    "versions": {
        "2.4.159": { "version":"2.4.159", "dist": { "shasum":"", "tarball":"" }},
        "3.2.42": { "version":"3.2.42", "dist": { "shasum":"", "tarball":"" }},
        "3.7.71": { "version":"3.7.71", "dist": { "shasum":"", "tarball":"" }},
        "3.12.99": { "version":"3.12.99", "dist": { "shasum":"", "tarball":"" }}
    }
}"#,
        )
        .create();

    assert_that!(
        s.volta("install yarn@3"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Using yarn.index hook to determine yarn index URL")
            .with_stderr_contains("[..]Found yarn@3.12.99 matching requirement[..]")
            .with_stderr_contains("[..]Could not download yarn@3.12.99")
    );
}

#[test]
fn hook_with_invalid_registry_format_errors() {
    let s = sandbox()
        .default_hooks(
            r#"{
    "yarn": {
        "index": {
            "prefix": "http://localhost/yarn/index/",
            "format": "invalid"
        }
    }
}"#,
        )
        .build();

    assert_that!(
        s.volta("install yarn@1.2.42"),
        execs()
            .with_status(ExitCode::ConfigurationError as i32)
            .with_stderr_contains("[..]Unrecognized index registry format: 'invalid'")
    );
}

#[test]
fn yarn_latest_hook_server_error() {
    let mut s = sandbox()
        .default_hooks(&yarn_hooks_json())
        .env("VOLTA_LOGLEVEL", "debug")
        .build();
    let _mock = s
        .server()
        .mock("GET", "/yarn-old/latest")
        .with_status(500)
        .create();

    assert_that!(
        s.volta("install yarn@latest"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not fetch latest version of Yarn")
    );
}

#[test]
fn node_registry_server_error() {
    let mut s = sandbox()
        .package_json(
            r#"{
  "name": "test-package",
  "volta": {
    "node": "10.99.1040"
  }
}"#,
        )
        .build();
    // Return 500 for the node index endpoint
    let _mock = s
        .server()
        .mock("GET", "/node-dist/index.json")
        .with_status(500)
        .create();

    assert_that!(
        s.volta("pin node@^10"),
        execs()
            .with_status(ExitCode::NetworkError as i32)
            .with_stderr_contains("[..]Could not download Node version registry")
    );
}
