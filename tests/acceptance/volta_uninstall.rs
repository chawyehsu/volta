//! Tests for `volta uninstall`.

use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use testkit::matchers::execs;
use testkit::sandbox::{Tool, sandbox};
use volta_core::constant;
use volta_core::error::ExitCode;
use volta_core::tool::node::HostPlatform;

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

fn package_json_with_pinned_node_yarn(node: &str, yarn: &str) -> String {
    format!(
        r#"{{
  "name": "test-project",
  "volta": {{
    "node": "{}",
    "yarn": "{}"
  }}
}}"#,
        node, yarn
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
fn uninstall_runtime_exact_removes_stale_archives() {
    // node distro archives are host-platform-specific; removing a version
    // should also remove archives fetched by a Volta binary for a different
    // platform, e.g. from before an Intel to Apple Silicon upgrade
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").image().create();

    let other = HostPlatform::ALL
        .into_iter()
        .find(|platform| *platform != HostPlatform::current())
        .unwrap();
    let stale = s.volta_home().join("tools/inventory/node").join(
        other
            .node_distro()
            .archive_filename(&"10.0.0".parse().unwrap()),
    );
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(&stale, "stale archive").unwrap();

    assert_that!(
        s.volta("uninstall node@10.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
    );

    assert!(!stale.exists());
}

#[test]
#[cfg(unix)]
fn uninstall_runtime_failure_keeps_metadata() {
    use std::os::unix::fs::PermissionsExt;

    // if the image cannot be deleted (e.g. a running binary on Windows), the
    // bundled npm metadata must remain: an image without its metadata would
    // be treated as cached but fail to load on the next run
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").npm_file().write("2.1.0");
    node.version("10.0.0").image().create();

    let image_dir = node.version("10.0.0").image();
    std::fs::set_permissions(&*image_dir, std::fs::Permissions::from_mode(0o000)).unwrap();

    // Skip when running with privileges that bypass file permissions (e.g.
    // root): the mode-000 directory is still readable, so the removal would
    // succeed instead of failing
    if std::fs::read_dir(&*image_dir).is_ok() {
        std::fs::set_permissions(&*image_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let output = match s.volta("uninstall node@10.0.0").exec_with_output() {
        Ok(output) => output,
        Err(err) => err.output.expect("ProcessError should contain output"),
    };

    // Restore permissions so sandbox cleanup can remove the directory
    std::fs::set_permissions(&*image_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_ne!(
        output.status.code(),
        Some(ExitCode::Success as i32),
        "Expected failure, got success"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Could not remove directory"),
        "Expected DeleteDirectoryError in stderr, got: {}",
        stderr
    );

    // the image and its metadata are untouched, keeping the version usable
    assert!(node.version("10.0.0").image().exists());
    assert!(node.version("10.0.0").npm_file().exists());
}

#[test]
#[cfg(unix)]
fn uninstall_runtime_failure_after_default_removal_unpins_first() {
    use std::os::unix::fs::PermissionsExt;

    // a broad removal that deletes the default and then fails on a later
    // version must still have unpinned the default, or a retry could not clear
    // it
    let s = sandbox()
        .platform(&platform_with_node("10.0.0"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").image().create();
    node.version("12.0.0").image().create();

    let broken = node.version("12.0.0").image();
    std::fs::set_permissions(&*broken, std::fs::Permissions::from_mode(0o000)).unwrap();

    if std::fs::read_dir(&*broken).is_ok() {
        std::fs::set_permissions(&*broken, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let output = match s.volta("uninstall node").exec_with_output() {
        Ok(output) => output,
        Err(err) => err.output.expect("ProcessError should contain output"),
    };

    std::fs::set_permissions(&*broken, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_ne!(
        output.status.code(),
        Some(ExitCode::Success as i32),
        "Expected failure, got success"
    );

    // the default was removed and unpinned before the failure
    assert!(!node.version("10.0.0").image().exists());
    assert_eq!(s.read_default_platform(), "{}");
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
fn uninstall_runtime_broad_removes_default_and_unpins() {
    // a bare tool name removes all cached versions, including the default,
    // and clears the default that was removed
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
            .with_stdout_contains("[..]'node@12.0.0' uninstalled")
            .with_stdout_contains("[..]'node@14.0.0' uninstalled")
            .with_stdout_contains("[..]unpinned 'node' from your default")
    );

    assert!(!node.version("10.0.0").image().exists());
    assert!(!node.version("12.0.0").image().exists());
    assert!(!node.version("14.0.0").image().exists());
    assert_eq!(s.read_default_platform(), "{}");
}

#[test]
fn uninstall_runtime_exact_default_unpins() {
    // removing the default version by exact targeting clears the default
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
            .with_stdout_contains("[..]unpinned 'node' from your default")
    );

    assert!(!s.tool(Tool::Node).version("12.0.0").image().exists());
    assert_eq!(s.read_default_platform(), "{}");
}

#[test]
fn uninstall_runtime_exact_other_version_keeps_default() {
    // removing a non-default version leaves the default untouched
    let s = sandbox()
        .platform(&platform_with_node("12.0.0"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").image().create();
    node.version("12.0.0").image().create();

    assert_that!(
        s.volta("uninstall node@10.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stdout_does_not_contain("unpinned 'node' from your default")
    );

    assert_eq!(s.read_default_platform(), platform_with_node("12.0.0"));
}

#[test]
fn uninstall_runtime_semver_removes_default_and_unpins() {
    // a range removes the default when it falls inside it, even though other
    // versions outside the range remain
    let s = sandbox()
        .platform(&platform_with_node("12.0.0"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("12.0.0").image().create();
    node.version("14.0.0").image().create();

    assert_that!(
        s.volta("uninstall node@12"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@12.0.0' uninstalled")
            .with_stdout_contains("[..]unpinned 'node' from your default")
    );

    assert!(!node.version("12.0.0").image().exists());
    assert!(node.version("14.0.0").image().exists());
    assert_eq!(s.read_default_platform(), "{}");
}

#[test]
fn uninstall_runtime_keeps_default_when_manager_defaulted() {
    // node's default is left in place while pnpm or yarn is also defaulted,
    // since a platform without node cannot carry them
    let s = sandbox()
        .platform(&platform_with_pnpm("10.99.1040", "7.7.1"))
        .env(constant::ENVNAME_VOLTA_FEATURE_PNPM, "1")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("10.99.1040").image().create();

    assert_that!(
        s.volta("uninstall node@10.99.1040"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.99.1040' uninstalled")
            .with_stdout_does_not_contain("unpinned 'node' from your default")
    );

    assert_eq!(
        s.read_default_platform(),
        platform_with_pnpm("10.99.1040", "7.7.1")
    );
}

#[test]
fn uninstall_runtime_keeps_default_when_npm_defaulted() {
    // an independent npm default lives inside the node entry, so removing node
    // must keep the node pin or npm would become unusable
    let s = sandbox()
        .platform(&platform_with_node_npm("10.99.1040", "6.14.13"))
        .shim("node")
        .shim("npx")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("10.99.1040").image().create();
    s.tool(Tool::Npm).version("6.14.13").image().create();

    assert_that!(
        s.volta("uninstall node@10.99.1040"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.99.1040' uninstalled")
            .with_stdout_does_not_contain("unpinned 'node' from your default")
    );

    assert_eq!(
        s.read_default_platform(),
        platform_with_node_npm("10.99.1040", "6.14.13")
    );
    // the retained default keeps the node shims so it can be re-fetched
    assert!(s.shim_exists("node"));
    assert!(s.shim_exists("npm"));
    assert!(s.shim_exists("npx"));
    assert!(s.shim_exe_exists("node"));
    assert!(s.shim_exe_exists("npm"));
    assert!(s.shim_exe_exists("npx"));
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
fn uninstall_last_version_removes_orphaned_shims() {
    // once a tool has no cached versions left, its shims are removed
    let s = sandbox()
        .without_default_shims()
        .shim("node")
        .shim("npm")
        .shim("npx")
        .shim("yarn")
        .shim("yarnpkg")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("10.0.0").image().create();
    s.tool(Tool::Yarn).version("1.0.0").image().create();

    assert!(s.shim_exists("node"));
    assert!(s.shim_exists("npm"));
    assert!(s.shim_exists("yarn"));
    assert!(s.shim_exe_exists("node"));
    assert!(s.shim_exe_exists("npm"));
    assert!(s.shim_exe_exists("npx"));
    assert!(s.shim_exe_exists("yarn"));
    assert!(s.shim_exe_exists("yarnpkg"));

    assert_that!(
        s.volta("uninstall node yarn"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stdout_contains("[..]'yarn@1.0.0' uninstalled")
    );

    assert!(!s.shim_exists("node"));
    assert!(!s.shim_exists("npm"));
    assert!(!s.shim_exists("npx"));
    assert!(!s.shim_exists("yarn"));
    assert!(!s.shim_exists("yarnpkg"));
    assert!(!s.shim_exe_exists("node"));
    assert!(!s.shim_exe_exists("npm"));
    assert!(!s.shim_exe_exists("npx"));
    assert!(!s.shim_exe_exists("yarn"));
    assert!(!s.shim_exe_exists("yarnpkg"));
}

#[test]
fn uninstall_keeps_shims_for_remaining_versions() {
    // a shim stays as long as another version of any tool still provides it
    let s = sandbox()
        .without_default_shims()
        .shim("node")
        .shim("npm")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").image().create();
    node.version("14.0.0").image().create();

    assert_that!(s.volta("uninstall node@10.0.0"), execs().with_status(0));

    assert!(s.shim_exists("node"));
    assert!(s.shim_exists("npm"));
    assert!(s.shim_exe_exists("node"));
    assert!(s.shim_exe_exists("npm"));
}

#[test]
fn uninstall_keeps_shims_for_project_pin() {
    // removing the only cached, project-pinned version by exact version must
    // keep the shims so the project pin can re-fetch it
    let s = sandbox()
        .package_json(&package_json_with_pinned_node("12.0.0"))
        .without_default_shims()
        .shim("node")
        .shim("npm")
        .shim("npx")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("12.0.0").image().create();

    assert_that!(s.volta("uninstall node@12.0.0"), execs().with_status(0));

    assert!(!s.tool(Tool::Node).version("12.0.0").image().exists());
    assert!(s.shim_exists("node"));
    assert!(s.shim_exists("npm"));
    assert!(s.shim_exists("npx"));
    assert!(s.shim_exe_exists("node"));
    assert!(s.shim_exe_exists("npm"));
    assert!(s.shim_exe_exists("npx"));
}

#[test]
fn uninstall_keeps_shims_for_project_pin_yarn() {
    // the same protection applies to project-pinned package managers
    let s = sandbox()
        .package_json(&package_json_with_pinned_node_yarn("12.0.0", "1.22.19"))
        .without_default_shims()
        .shim("yarn")
        .shim("yarnpkg")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Yarn).version("1.22.19").image().create();

    assert_that!(s.volta("uninstall yarn@1.22.19"), execs().with_status(0));

    assert!(s.shim_exists("yarn"));
    assert!(s.shim_exists("yarnpkg"));
    assert!(s.shim_exe_exists("yarn"));
    assert!(s.shim_exe_exists("yarnpkg"));
}

#[test]
fn uninstall_keeps_shims_for_package_platform() {
    // a global package records its install-time platform, which still needs
    // Volta dispatch after the matching runtime version is uninstalled
    let s = sandbox()
        .package_config("cowsay", PKG_CONFIG_BASIC)
        .without_default_shims()
        .shim("node")
        .shim("npm")
        .shim("npx")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("11.10.1").image().create();

    assert_that!(s.volta("uninstall node@11.10.1"), execs().with_status(0));

    assert!(s.shim_exists("node"));
    assert!(s.shim_exists("npm"));
    assert!(s.shim_exists("npx"));
    assert!(s.shim_exe_exists("node"));
    assert!(s.shim_exe_exists("npm"));
    assert!(s.shim_exe_exists("npx"));
}

#[test]
fn uninstall_node_keeps_npm_shims_when_npm_cached() {
    // node provides npm and npx too, so those shims survive node's removal
    // while a standalone npm is still cached
    let s = sandbox()
        .without_default_shims()
        .shim("node")
        .shim("npm")
        .shim("npx")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("10.0.0").image().create();
    s.tool(Tool::Npm).version("6.14.13").image().create();

    assert_that!(s.volta("uninstall node"), execs().with_status(0));

    assert!(!s.shim_exists("node"));
    assert!(s.shim_exists("npm"));
    assert!(s.shim_exists("npx"));
    assert!(!s.shim_exe_exists("node"));
    assert!(s.shim_exe_exists("npm"));
    assert!(s.shim_exe_exists("npx"));
}

#[test]
fn uninstall_npm_keeps_shims_when_node_cached() {
    // npm shares npm and npx with node, so its shims survive while node is
    // still cached
    let s = sandbox()
        .without_default_shims()
        .shim("npm")
        .shim("npx")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("10.0.0").image().create();
    s.tool(Tool::Npm).version("6.14.13").image().create();

    assert_that!(s.volta("uninstall npm"), execs().with_status(0));

    assert!(s.shim_exists("npm"));
    assert!(s.shim_exists("npx"));
    assert!(s.shim_exe_exists("npm"));
    assert!(s.shim_exe_exists("npx"));
}

#[test]
fn uninstall_npm_keeps_shims_when_node_default_remains() {
    // an independent node+npm default: uninstalling node retains the node pin
    // (for npm), then uninstalling the last npm leaves the node default, which
    // still supplies bundled npm even though no node image is cached
    let s = sandbox()
        .platform(&platform_with_node_npm("10.99.1040", "6.14.13"))
        .shim("node")
        .shim("npx")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("10.99.1040").image().create();
    s.tool(Tool::Npm).version("6.14.13").image().create();

    assert_that!(s.volta("uninstall node@10.99.1040"), execs().with_status(0));
    assert_eq!(
        s.read_default_platform(),
        platform_with_node_npm("10.99.1040", "6.14.13")
    );

    assert_that!(s.volta("uninstall npm@6.14.13"), execs().with_status(0));
    assert_eq!(s.read_default_platform(), platform_with_node("10.99.1040"));

    assert!(s.shim_exists("npm"));
    assert!(s.shim_exists("npx"));
    assert!(s.shim_exe_exists("npm"));
    assert!(s.shim_exe_exists("npx"));
}

#[test]
fn uninstall_exact_tolerates_malformed_platform_after_removal() {
    // an exact removal tolerates a malformed platform.json during in-use
    // collection; the post-removal shim cleanup must tolerate it too instead of
    // failing after the version is already gone
    let s = sandbox()
        .platform("this is not json{{{")
        .shim("node")
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    s.tool(Tool::Node).version("10.0.0").image().create();

    assert_that!(
        s.volta("uninstall node@10.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stderr_contains("[..]skipping shim cleanup")
    );

    assert!(!s.tool(Tool::Node).version("10.0.0").image().exists());
    // cleanup was skipped, so the shim is left in place
    assert!(s.shim_exists("node"));
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
fn uninstall_npm_broad_removes_default_and_unpins() {
    // a bare `npm` removes all cached versions, including the default, and
    // clears npm from the default platform (falling back to bundled npm)
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
            .with_stdout_contains("[..]'npm@6.2.26' uninstalled")
            .with_stdout_contains("[..]unpinned 'npm' from your default")
    );

    assert!(!npm.version("6.14.13").image().exists());
    assert!(!npm.version("6.2.26").image().exists());
    assert_eq!(s.read_default_platform(), platform_with_node("10.99.1040"));
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
fn uninstall_pnpm_broad_removes_default_and_unpins() {
    // a bare `pnpm` removes all cached versions, including the default, and
    // clears pnpm from the default platform
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
            .with_stdout_contains("[..]'pnpm@7.7.1' uninstalled")
            .with_stdout_contains("[..]unpinned 'pnpm' from your default")
    );

    assert!(!pnpm.version("8.0.0").image().exists());
    assert!(!pnpm.version("7.7.1").image().exists());
    assert_eq!(s.read_default_platform(), platform_with_node("10.99.1040"));
}

#[test]
fn uninstall_pnpm_without_feature_flag_falls_back_to_package() {
    // without the pnpm feature flag, pnpm is treated as a global package,
    // which does not support versioned uninstalls
    let s = sandbox()
        .env_remove(constant::ENVNAME_VOLTA_FEATURE_PNPM)
        .env(VOLTA_LOGLEVEL, "info")
        .build();

    assert_that!(
        s.volta("uninstall pnpm@7.1.0"),
        execs().with_status(1).with_stderr_contains(
            "[..]error: uninstalling specific versions of tools is not supported yet."
        )
    );

    // the bare package fallback is unchanged
    assert_that!(
        s.volta("uninstall pnpm"),
        execs()
            .with_status(0)
            .with_stderr_contains("[..]No package 'pnpm' found to uninstall")
    );
}

#[test]
fn uninstall_yarn_exact() {
    // uninstalling an exact yarn version removes the unpacked image and the
    // cached archive
    let s = sandbox().env(VOLTA_LOGLEVEL, "info").build();
    let yarn = s.tool(Tool::Yarn);
    yarn.version("1.22.19").image().create();
    yarn.version("1.22.19").archive().write("yarn tarball");
    yarn.version("1.22.5").image().create();

    assert!(yarn.version("1.22.19").archive().exists());

    assert_that!(
        s.volta("uninstall yarn@1.22.19"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'yarn@1.22.19' uninstalled")
    );

    assert!(!yarn.version("1.22.19").image().exists());
    assert!(!yarn.version("1.22.19").archive().exists());
    // other versions are untouched
    assert!(yarn.version("1.22.5").image().exists());
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
fn uninstall_runtime_exact_warns_project_pin() {
    // removing a project-pinned version by exact targeting warns that it
    // will be re-downloaded on next use; the pin itself is untouched
    let s = sandbox()
        .package_json(&package_json_with_pinned_node("12.0.0"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("12.0.0").image().create();

    assert_that!(
        s.volta("uninstall node@12.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@12.0.0' uninstalled")
            .with_stderr_contains("[..]'node@12.0.0' is pinned in the current project; it will be re-downloaded on next use")
    );

    assert!(!node.version("12.0.0").image().exists());
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
fn uninstall_runtime_exact_tolerates_broken_package_config() {
    // a malformed global-package config should not block a version the user
    // named exactly: references that can still be read (the default, and the
    // other packages' configs) are preserved for the warnings, while a broad
    // removal fails loudly instead of guessing
    let s = sandbox()
        .platform(&platform_with_node("10.0.0"))
        .package_config("broken", "this is not json{{{")
        .package_config("cowsay", &node_package_config("cowsay"))
        .env(VOLTA_LOGLEVEL, "info")
        .build();
    let node = s.tool(Tool::Node);
    node.version("10.0.0").image().create();
    node.version("11.10.1").image().create();

    assert_that!(
        s.volta("uninstall node@11.10.1"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@11.10.1' uninstalled")
            .with_stderr_contains(
                "[..]Unable to fully determine in-use versions (Could not parse package configuration file.[..]"
            )
            .with_stderr_contains("[..]removing 'node@11.10.1' anyway")
            .with_stderr_contains("[..]'node@11.10.1' is used by global package 'cowsay'; it will be re-downloaded the next time 'cowsay' runs")
    );
    assert!(!node.version("11.10.1").image().exists());

    // the readable default is still unpinned even though a package config broke
    assert_that!(
        s.volta("uninstall node@10.0.0"),
        execs()
            .with_status(0)
            .with_stdout_contains("[..]'node@10.0.0' uninstalled")
            .with_stdout_contains("[..]unpinned 'node' from your default")
    );
    assert!(!node.version("10.0.0").image().exists());
    assert_eq!(s.read_default_platform(), "{}");

    // broad removals stay strict
    node.version("12.0.0").image().create();
    assert_that!(
        s.volta("uninstall node"),
        execs()
            .with_status(1)
            .with_stderr_contains("[..]Could not parse package configuration file.")
    );
    assert!(node.version("12.0.0").image().exists());
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
