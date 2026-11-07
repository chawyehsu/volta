//! Tests for `volta uninstall`.

use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use testkit::matchers::execs;
use testkit::sandbox::{Tool, sandbox};
use volta_core::constant;
use volta_core::error::ExitCode;

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

const PKG_CONFIG_NO_BINS: &str = r#"{
  "name": "cowsay",
  "version": "1.4.0",
  "platform": {
    "node": "11.10.1",
    "npm": "6.7.0",
    "yarn": null
  },
  "bins": [],
  "manager": "Npm"
}"#;

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

fn package_json_with_pinned_node(node: &str) -> String {
    format!(
        r#"{{
  "name": "test-project",
  "volta": {{
    "node": "{}"
  }}
}}"#,
        node
    )
}

fn node_package_config(name: &str) -> String {
    format!(
        r#"{{
  "name": "{}",
  "version": "1.4.0",
  "platform": {{
    "node": "11.10.1",
    "npm": "6.7.0",
    "yarn": null
  }},
  "bins": [],
  "manager": "Npm"
}}"#,
        name
    )
}

const VOLTA_LOGLEVEL: &str = "VOLTA_LOGLEVEL";

#[test]
fn uninstall_nonexistent_pkg() {
    // if the package doesn't exist, it should just inform the user but not throw an error
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();

    assert_that!(
        s.volta("uninstall cowsay"),
        execs()
            .with_status(0)
            .with_stderr_contains("[..]No package 'cowsay' found to uninstall")
    );
}

#[test]
fn uninstall_package_basic() {
    // basic uninstall - everything exists, and everything except the cached
    // inventory files should be deleted
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .package_image("cowsay", "1.4.0", None)
        .env(VOLTA_LOGLEVEL, "info")
        .build();

    assert_that!(
        s.volta("uninstall cowsay"),
        execs()
            .with_status(0)
            .with_stdout_contains("Removed executable 'cowsay' installed by 'cowsay'")
            .with_stdout_contains("Removed executable 'cowthink' installed by 'cowsay'")
            .with_stdout_contains("[..]package 'cowsay' uninstalled")
    );

    // check that everything is deleted
    assert!(!s.package_config_exists("cowsay"));
    assert!(!s.bin_config_exists("cowsay"));
    assert!(!s.bin_config_exists("cowthink"));
    assert!(!s.shim_exists("cowsay"));
    assert!(!s.shim_exists("cowthink"));
    assert!(!s.package_image_exists("cowsay"));
}

// The setup here is the same as the above, but here we check to make sure that
// if the user supplies a version, we error correctly.
#[test]
fn uninstall_package_basic_with_version() {
    // basic uninstall - everything exists, and everything except the cached
    // inventory files should be deleted
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .package_image("cowsay", "1.4.0", None)
        .env(VOLTA_LOGLEVEL, "info")
        .build();

    assert_that!(
        s.volta("uninstall cowsay@1.4.0"),
        execs().with_status(1).with_stderr_contains(
            "[..]error: uninstalling specific versions of tools is not supported yet."
        )
    );
}

#[test]
fn uninstall_package_no_bins() {
    // the package doesn't contain any executables, it should uninstall without error
    // (normally installing a package with no executables should not happen)
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_NO_BINS)
        .package_image("cowsay", "1.4.0", None)
        .env(VOLTA_LOGLEVEL, "info")
        .build();

    assert_that!(
        s.volta("uninstall cowsay"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]package 'cowsay' uninstalled")
    );

    // check that everything is deleted
    assert!(!s.package_config_exists("cowsay"));
    assert!(!s.bin_config_exists("cowsay"));
    assert!(!s.bin_config_exists("cowthink"));
    assert!(!s.shim_exists("cowsay"));
    assert!(!s.shim_exists("cowthink"));
    assert!(!s.package_image_exists("cowsay"));
}

#[test]
fn uninstall_package_no_image() {
    // there is no unpacked & initialized package, but everything should be removed
    // (without erroring and failing to remove everything)
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .env(VOLTA_LOGLEVEL, "info")
        .build();

    assert_that!(
        s.volta("uninstall cowsay"),
        execs()
            .with_status(0)
            .with_stdout_contains("Removed executable 'cowsay' installed by 'cowsay'")
            .with_stdout_contains("Removed executable 'cowthink' installed by 'cowsay'")
            .with_stdout_contains("[..]package 'cowsay' uninstalled")
    );

    // check that everything is deleted
    assert!(!s.package_config_exists("cowsay"));
    assert!(!s.bin_config_exists("cowsay"));
    assert!(!s.bin_config_exists("cowthink"));
    assert!(!s.shim_exists("cowsay"));
    assert!(!s.shim_exists("cowthink"));
    assert!(!s.package_image_exists("cowsay"));
}

#[test]
fn uninstall_package_orphaned_bins() {
    // the package config does not exist, but for some reason there are orphaned binaries
    // those should be removed
    let s = sandbox()
        .binary_config("cowsay", &bin_config("cowsay"))
        .binary_config("cowthink", &bin_config("cowthink"))
        .shim("cowsay")
        .shim("cowthink")
        .env(VOLTA_LOGLEVEL, "info")
        .build();

    assert_that!(
        s.volta("uninstall cowsay"),
        execs()
            .with_status(0)
            .with_stdout_contains("Removed executable 'cowsay' installed by 'cowsay'")
            .with_stdout_contains("Removed executable 'cowthink' installed by 'cowsay'")
            .with_stdout_contains("[..]package 'cowsay' uninstalled")
    );

    // check that everything is deleted
    assert!(!s.bin_config_exists("cowsay"));
    assert!(!s.bin_config_exists("cowthink"));
    assert!(!s.shim_exists("cowsay"));
    assert!(!s.shim_exists("cowthink"));
}

#[test]
fn uninstall_runtime_not_found() {
    // if there are no cached versions of a runtime, it should just inform the
    // user but not throw an error
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();

    assert_that!(
        s.volta("uninstall node"),
        execs()
            .with_status(0)
            .with_stderr_contains("[..]No cached versions of 'node' found to uninstall")
    );
}

#[test]
fn uninstall_runtime_exact() {
    // uninstalling an exact version removes the unpacked image, the cached
    // archive, and the bundled npm metadata file
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").archive().write("node tarball");
    node.version("10.0.0").npm_file().write("2.1.0");
    node.version("10.0.0").image().create();
    node.version("12.0.0").image().create();

    assert!(node.version("10.0.0").archive().exists());

    assert_that!(
        s.volta("uninstall node@10.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
    );

    assert!(!node.version("10.0.0").image().exists());
    assert!(!node.version("10.0.0").archive().exists());
    assert!(!node.version("10.0.0").npm_file().exists());
    // other versions are untouched
    assert!(node.version("12.0.0").image().exists());
}

#[test]
fn uninstall_runtime_semver() {
    // uninstalling a version range removes all cached versions matching it
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").image().create();
    node.version("10.9.9").image().create();
    node.version("11.0.0").image().create();

    assert_that!(
        s.volta("uninstall node@10"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stdout_contains("[..]'node@10.9.9' uninstalled")
    );

    assert!(!node.version("10.0.0").image().exists());
    assert!(!node.version("10.9.9").image().exists());
    // versions outside the range are untouched
    assert!(node.version("11.0.0").image().exists());
}

#[test]
fn uninstall_runtime_broad_skips_default() {
    // a bare tool name removes all cached versions except the default
    let s = sandbox()
        .platform(&platform_with_node("12.0.0"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").image().create();
    node.version("12.0.0").image().create();
    node.version("14.0.0").image().create();

    assert_that!(
        s.volta("uninstall node"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stdout_contains("[..]'node@14.0.0' uninstalled")
            .with_stderr_contains("[..]skipping 'node@12.0.0' (currently your default node; target it with an exact version to remove it)")
    );

    assert!(!node.version("10.0.0").image().exists());
    assert!(!node.version("14.0.0").image().exists());
    assert!(node.version("12.0.0").image().exists());
    // uninstall does not change the user's default
    assert_eq!(s.read_default_platform(), platform_with_node("12.0.0"));
}

#[test]
fn uninstall_runtime_exact_in_use() {
    // an in-use version can be removed by targeting it explicitly; its config
    // is left untouched, since the version will be re-fetched on next use
    let s = sandbox()
        .platform(&platform_with_node("12.0.0"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("12.0.0").image().create();

    assert_that!(
        s.volta("uninstall node@12.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@12.0.0' uninstalled")
            .with_stderr_contains("[..]'node@12.0.0' is your default node; it will be re-downloaded on next use. To change your default, run `volta install node@<version>`")
    );

    assert!(!s.tool(Tool::Node).version("12.0.0").image().exists());
    assert_eq!(s.read_default_platform(), platform_with_node("12.0.0"));
}

#[test]
fn uninstall_runtime_broad_skips_package_dependency() {
    // a version depended on by an installed global package is skipped
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("11.10.1").image().create();
    node.version("10.0.0").image().create();

    assert_that!(
        s.volta("uninstall node"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stderr_contains("[..]skipping 'node@11.10.1' (in use by global package 'cowsay'; target it with an exact version to remove it)")
    );

    assert!(!node.version("10.0.0").image().exists());
    assert!(node.version("11.10.1").image().exists());
}

#[test]
fn uninstall_runtime_tag_unsupported() {
    // tag-based specs would require resolving against the remote index
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();

    assert_that!(
        s.volta("uninstall node@latest"),
        execs()
            .with_status(ExitCode::InvalidArguments as i32)
            .with_stderr_contains(
                "[..]error: uninstalling 'node@latest' is not supported; specify an exact version or a version range"
            )
    );
}

#[test]
fn uninstall_runtime_multiple_tools() {
    // multiple tools can be uninstalled in a single command
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();
    s.tool(Tool::Node).version("10.0.0").image().create();
    s.tool(Tool::Yarn).version("1.0.0").image().create();

    assert_that!(
        s.volta("uninstall node@10.0.0 yarn@1.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stdout_contains("[..]'yarn@1.0.0' uninstalled")
    );

    assert!(!s.tool(Tool::Node).version("10.0.0").image().exists());
    assert!(!s.tool(Tool::Yarn).version("1.0.0").image().exists());
}

#[test]
fn uninstall_npm_exact() {
    // uninstalling an exact npm version removes the unpacked image and the
    // cached archive
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();
    let npm = s.tool(Tool::Npm);
    npm.version("6.14.13").image().create();
    npm.version("6.14.13").archive().write("npm tarball");
    npm.version("6.2.26").image().create();

    assert!(npm.version("6.14.13").archive().exists());

    assert_that!(
        s.volta("uninstall npm@6.14.13"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'npm@6.14.13' uninstalled")
    );

    assert!(!npm.version("6.14.13").image().exists());
    assert!(!npm.version("6.14.13").archive().exists());
    // other versions are untouched
    assert!(npm.version("6.2.26").image().exists());
}

#[test]
fn uninstall_npm_broad_skips_default() {
    // a bare `npm` removes all cached versions except the default
    let s = sandbox()
        .platform(&platform_with_node_npm("10.99.1040", "6.2.26"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let npm = s.tool(Tool::Npm);
    npm.version("6.2.26").image().create();
    npm.version("6.14.13").image().create();

    assert_that!(
        s.volta("uninstall npm"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'npm@6.14.13' uninstalled")
            .with_stderr_contains("[..]skipping 'npm@6.2.26' (currently your default npm; target it with an exact version to remove it)")
    );

    assert!(!npm.version("6.14.13").image().exists());
    assert!(npm.version("6.2.26").image().exists());
    assert_eq!(
        s.read_default_platform(),
        platform_with_node_npm("10.99.1040", "6.2.26")
    );
}

#[test]
fn uninstall_npm_broad_skips_package_dependency() {
    // an npm version depended on by an installed global package is skipped
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let npm = s.tool(Tool::Npm);
    npm.version("6.7.0").image().create();
    npm.version("6.14.13").image().create();

    assert_that!(
        s.volta("uninstall npm"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'npm@6.14.13' uninstalled")
            .with_stderr_contains("[..]skipping 'npm@6.7.0' (in use by global package 'cowsay'; target it with an exact version to remove it)")
    );

    assert!(!npm.version("6.14.13").image().exists());
    assert!(npm.version("6.7.0").image().exists());
}

#[test]
fn uninstall_pnpm_exact() {
    // with the pnpm feature flag enabled, an exact pnpm version can be
    // uninstalled like the other package managers
    let s = sandbox()
        .env(constant::ENVNAME_VOLTA_FEATURE_PNPM, "1")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let pnpm = s.tool(Tool::Pnpm);
    pnpm.version("7.1.0").image().create();
    pnpm.version("7.1.0").archive().write("pnpm tarball");
    pnpm.version("6.32.0").image().create();

    assert!(pnpm.version("7.1.0").archive().exists());

    assert_that!(
        s.volta("uninstall pnpm@7.1.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'pnpm@7.1.0' uninstalled")
    );

    assert!(!pnpm.version("7.1.0").image().exists());
    assert!(!pnpm.version("7.1.0").archive().exists());
    assert!(pnpm.version("6.32.0").image().exists());
}

#[test]
fn uninstall_pnpm_broad_skips_default() {
    // a bare `pnpm` removes all cached versions except the default
    let s = sandbox()
        .platform(&platform_with_pnpm("10.99.1040", "7.7.1"))
        .env(constant::ENVNAME_VOLTA_FEATURE_PNPM, "1")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let pnpm = s.tool(Tool::Pnpm);
    pnpm.version("7.7.1").image().create();
    pnpm.version("8.0.0").image().create();

    assert_that!(
        s.volta("uninstall pnpm"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'pnpm@8.0.0' uninstalled")
            .with_stderr_contains("[..]skipping 'pnpm@7.7.1' (currently your default pnpm; target it with an exact version to remove it)")
    );

    assert!(!pnpm.version("8.0.0").image().exists());
    assert!(pnpm.version("7.7.1").image().exists());
    assert_eq!(
        s.read_default_platform(),
        platform_with_pnpm("10.99.1040", "7.7.1")
    );
}

#[test]
fn uninstall_pnpm_without_feature_flag_falls_back_to_package() {
    // without the pnpm feature flag, pnpm is treated as a global package
    let s = sandbox()
        .env_remove(constant::ENVNAME_VOLTA_FEATURE_PNPM)
        .env(VOLTA_LOGLEVEL, "info")
        .build();

    assert_that!(
        s.volta("uninstall pnpm@7.1.0"),
        execs()
            .with_status(0)
            .with_stderr_contains("[..]No package 'pnpm' found to uninstall")
    );
}

#[test]
fn uninstall_runtime_exact_warns_package_dependency() {
    // removing a package-dependent version by exact targeting warns that it
    // will be re-downloaded the next time the package runs
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("11.10.1").image().create();

    assert_that!(
        s.volta("uninstall node@11.10.1"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@11.10.1' uninstalled")
            .with_stderr_contains("[..]'node@11.10.1' is used by global package 'cowsay'; it will be re-downloaded the next time 'cowsay' runs")
    );

    assert!(!node.version("11.10.1").image().exists());
}

#[test]
fn uninstall_runtime_broad_skips_project_pin() {
    // a version pinned in the current project is skipped by a bare tool name
    let s = sandbox()
        .package_json(&package_json_with_pinned_node("12.0.0"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("12.0.0").image().create();
    node.version("10.0.0").image().create();

    assert_that!(
        s.volta("uninstall node"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stderr_contains("[..]skipping 'node@12.0.0' (pinned in the current project; target it with an exact version to remove it)")
    );

    assert!(!node.version("10.0.0").image().exists());
    assert!(node.version("12.0.0").image().exists());
    // uninstall does not rewrite the project pin
    assert_eq!(
        s.read_package_json(),
        package_json_with_pinned_node("12.0.0")
    );
}

#[test]
fn uninstall_runtime_broad_skips_multiple_package_dependencies() {
    // when several packages depend on the same version, all are named
    let s = sandbox()
        .package_config("cowsay", &node_package_config("cowsay"))
        .package_config("eslint", &node_package_config("eslint"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("11.10.1").image().create();

    assert_that!(
        s.volta("uninstall node"),
        execs()
            .with_status(0)
            .with_stderr_contains("[..]skipping 'node@11.10.1' (in use by global packages 'cowsay', 'eslint'; target it with an exact version to remove it)")
    );

    assert!(node.version("11.10.1").image().exists());
}

#[test]
fn uninstall_bare_version_argument() {
    // `volta uninstall <version>` is not a valid invocation, same as install
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();

    assert_that!(
        s.volta("uninstall 12"),
        execs()
            .with_status(ExitCode::InvalidArguments as i32)
            .with_stderr_contains("[..]`volta uninstall 12` is not supported.")
    );
}

#[test]
#[cfg(unix)]
fn read_bin_config_dir_error() {
    use std::os::unix::fs::PermissionsExt;

    // Set up orphaned binaries (no package config) so that uninstall
    // reads the bins directory via binaries_from_package()
    let s = sandbox()
        .binary_config("cowsay", &bin_config("cowsay"))
        .build();

    // Make the bins directory unreadable
    let volta_home = s.volta_home();
    let bins_dir = volta_home.join("tools/user/bins");
    std::fs::set_permissions(&bins_dir, std::fs::Permissions::from_mode(0o000)).unwrap();

    let output = match s.volta("uninstall cowsay").exec_with_output() {
        Ok(output) => output,
        Err(err) => err.output.expect("ProcessError should contain output"),
    };

    // Restore permissions so sandbox cleanup can remove the directory
    std::fs::set_permissions(&bins_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_ne!(
        output.status.code(),
        Some(ExitCode::Success as i32),
        "Expected failure, got success"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Could not read executable metadata directory"),
        "Expected ReadBinConfigDirError in stderr, got: {}",
        stderr
    );
}

#[test]
fn shim_remove_error() {
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .binary_config("cowsay", &bin_config("cowsay"))
        .shim("cowsay")
        .build();

    let volta_home = s.volta_home();
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            // Replace the shim symlink with a directory so that fs::remove_file
            // fails with IsADirectory (not NotFound), triggering ShimRemoveError.
            // The migration skips non-symlink entries, so this won't break migration.
            let shim_path = volta_home.join("bin/cowsay");
            std::fs::remove_file(&shim_path).unwrap();
            std::fs::create_dir(&shim_path).unwrap();
        } else if #[cfg(windows)] {
            // Replace the .cmd shim with a directory so that fs::remove_file
            // fails with PermissionDenied (not NotFound), triggering ShimRemoveError.
            // The sandbox builder creates the `.cmd` shim (mirroring real Volta),
            // so remove it before creating the directory in its place.
            let shim_path = volta_home.join("bin/cowsay.cmd");
            std::fs::remove_file(&shim_path).unwrap();
            std::fs::create_dir(&shim_path).unwrap();
        }
    }

    let output = match s.volta("uninstall cowsay").exec_with_output() {
        Ok(output) => output,
        Err(err) => err.output.expect("ProcessError should contain output"),
    };

    assert_ne!(
        output.status.code(),
        Some(ExitCode::Success as i32),
        "Expected failure, got success"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Could not remove shim for \"cowsay\""),
        "Expected ShimRemoveError in stderr, got: {}",
        stderr
    );
}
