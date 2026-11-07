use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use testkit::matchers::execs;
use testkit::sandbox::{Tool, sandbox};

use volta_core::constant;
use volta_core::error::ExitCode;

#[test]
fn empty_volta_home_is_created() {
    let s = sandbox().build();

    // clear out the .volta dir
    s.remove_volta_home();

    // VOLTA_HOME starts out non-existent, with no shims
    assert!(!s.path_exists(".volta"));
    assert!(!s.shim_exists("node"));

    // running volta triggers automatic creation
    assert_that!(s.volta("--version"), execs().with_status(0));

    // home directories should all be created
    assert!(s.path_exists(".volta"));
    assert!(s.path_exists(".volta/bin"));
    assert!(s.path_exists(".volta/cache/node"));
    assert!(s.path_exists(".volta/log"));
    assert!(s.path_exists(".volta/tmp"));
    assert!(s.path_exists(".volta/tools/image/node"));
    assert!(s.path_exists(".volta/tools/image/yarn"));
    assert!(s.path_exists(".volta/tools/inventory/node"));
    assert!(s.path_exists(".volta/tools/inventory/yarn"));
    assert!(s.path_exists(".volta/tools/user"));

    // Layout file should now exist
    assert!(s.path_exists(".volta/layout.v4"));

    // shims should all be created
    // NOTE: this doesn't work in Windows, because the default shims are stored separately
    #[cfg(unix)]
    {
        ["node", "yarn", "npm", "npx"].iter().for_each(|shim| {
            assert!(s.shim_exists(shim));
        });
    }
}

#[test]
fn legacy_v0_volta_home_is_upgraded() {
    let s = sandbox().without_layout_marker().build();

    // directories that are already created by the test framework
    assert!(s.path_exists(".volta"));
    assert!(s.path_exists(".volta/cache/node"));
    assert!(s.path_exists(".volta/tmp"));
    assert!(s.path_exists(".volta/tools/inventory/node"));
    assert!(s.path_exists(".volta/tools/inventory/packages"));
    assert!(s.path_exists(".volta/tools/inventory/yarn"));

    // Layout file is not there
    assert!(!s.path_exists(".volta/layout.v1"));
    assert!(!s.path_exists(".volta/layout.v2"));
    assert!(!s.path_exists(".volta/layout.v3"));

    // running volta should not create anything else
    assert_that!(s.volta("--version"), execs().with_status(0));

    // Layout should be updated to the most recent
    assert!(s.path_exists(".volta"));
    assert!(s.path_exists(".volta/cache/node"));
    assert!(s.path_exists(".volta/tmp"));
    assert!(s.path_exists(".volta/tools/inventory/node"));
    assert!(!s.path_exists(".volta/tools/inventory/packages"));
    assert!(s.path_exists(".volta/tools/inventory/yarn"));

    // Most recent layout file should exist, others should not
    assert!(!s.path_exists(".volta/layout.v1"));
    assert!(!s.path_exists(".volta/layout.v2"));
    assert!(!s.path_exists(".volta/layout.v3"));
    assert!(s.path_exists(".volta/layout.v4"));

    // shims should all be created
    // NOTE: this doesn't work in Windows, because the default shims are stored separately
    #[cfg(unix)]
    {
        ["node", "yarn", "npm", "npx"].iter().for_each(|shim| {
            assert!(s.shim_exists(shim));
        });
    }
}

#[test]
fn tagged_v1_volta_home_is_upgraded() {
    let s = sandbox()
        .without_layout_marker()
        .layout_file("v1")
        .file(
            ".volta/tools/image/node/10.6.0/6.1.0/README.md",
            "Irrelevant Contents",
        )
        .platform(
            r#"{
            "node": {
                "runtime": "10.6.0",
                "npm": "6.1.0"
            },
            "yarn": null
        }"#,
        )
        .build();
    s.tool(Tool::Node)
        .version("10.6.0")
        .npm_file()
        .write("6.1.0");

    // We are already tagged as a v1 layout
    assert!(s.path_exists(".volta/layout.v1"));

    // Node image directory exists
    assert!(s.path_exists(".volta/tools/image/node/10.6.0/6.1.0/README.md"));
    assert!(s.path_exists(".volta/tools/inventory/node/node-v10.6.0-npm"));

    // Default platform includes npm version
    assert!(s.read_default_platform().contains(r#""npm": "6.1.0""#));

    // running volta should run the migration
    assert_that!(s.volta("--version"), execs().with_status(0));

    // Default platform should not include an npm version
    assert!(s.read_default_platform().contains(r#""npm": null"#));

    // Node image directory should be moved up and no longer contain the npm version
    assert!(s.path_exists(".volta/tools/image/node/10.6.0/README.md"));

    // Directory structure should exist
    assert!(s.path_exists(".volta"));
    assert!(s.path_exists(".volta/cache/node"));
    assert!(s.path_exists(".volta/tmp"));
    assert!(s.path_exists(".volta/tools/inventory/node"));
    assert!(s.path_exists(".volta/tools/inventory/yarn"));

    // Most recent layout file should exist, others should not
    assert!(!s.path_exists(".volta/layout.v1"));
    assert!(!s.path_exists(".volta/layout.v2"));
    assert!(!s.path_exists(".volta/layout.v3"));
    assert!(s.path_exists(".volta/layout.v4"));

    // shims should all be created
    // NOTE: this doesn't work in Windows, because the default shims are stored separately
    #[cfg(unix)]
    {
        ["node", "yarn", "npm", "npx"].iter().for_each(|shim| {
            assert!(s.shim_exists(shim));
        });
    }
}

#[test]
fn tagged_v1_to_v2_keeps_custom_npm() {
    let s = sandbox()
        .without_layout_marker()
        .layout_file("v1")
        .platform(
            r#"{
            "node": {
                "runtime": "10.6.0",
                "npm": "6.3.0"
            },
            "yarn": null
        }"#,
        )
        .build();
    s.tool(Tool::Node)
        .version("10.6.0")
        .npm_file()
        .write("6.1.0");

    // Default platform includes npm version
    assert!(s.read_default_platform().contains(r#""npm": "6.3.0""#));

    // running volta should run the migration
    assert_that!(s.volta("--version"), execs().with_status(0));

    // Default platform still includes custom npm version
    assert!(s.read_default_platform().contains(r#""npm": "6.3.0""#));
}

#[test]
fn tagged_v1_to_v2_keeps_migrated_node_images() {
    let s = sandbox()
        .without_layout_marker()
        .layout_file("v1")
        .file(
            ".volta/tools/image/node/10.6.0/README.md",
            "Irrelevant Contents",
        )
        .build();
    s.tool(Tool::Node)
        .version("10.6.0")
        .npm_file()
        .write("6.1.0");

    // Migrated Node image directory exists
    assert!(s.path_exists(".volta/tools/image/node/10.6.0/README.md"));

    // running volta should run the migration
    assert_that!(s.volta("--version"), execs().with_status(0));

    // Migrated Node image directory is unchanged
    assert!(s.path_exists(".volta/tools/image/node/10.6.0/README.md"));
}

#[test]
fn current_v4_volta_home_is_unchanged() {
    let s = sandbox().layout_file("v4").build();

    // directories that are already created by the test framework
    assert!(s.path_exists(".volta"));
    assert!(s.path_exists(".volta/layout.v4"));
    assert!(s.path_exists(".volta/cache/node"));
    assert!(s.path_exists(".volta/tmp"));
    assert!(s.path_exists(".volta/tools/inventory/node"));
    assert!(s.path_exists(".volta/tools/inventory/yarn"));

    // running volta should not create anything else
    assert_that!(s.volta("--version"), execs().with_status(0));

    // everything should be the same as before running the command
    assert!(s.path_exists(".volta"));
    assert!(s.path_exists(".volta/layout.v4"));
    assert!(s.path_exists(".volta/cache/node"));
    assert!(s.path_exists(".volta/tmp"));
    assert!(s.path_exists(".volta/tools/inventory/node"));
    assert!(s.path_exists(".volta/tools/inventory/yarn"));
}

#[test]
fn missing_migrate_executable_errors() {
    let builder = sandbox();
    let broken_install_dir = builder.root().join("broken-install");

    let s = builder
        .env(
            constant::ENVNAME_VOLTA_INSTALL_DIR,
            &broken_install_dir.to_string_lossy(),
        )
        .build();

    s.remove_volta_home();

    assert_that!(
        s.volta("--version"),
        execs()
            .with_status(ExitCode::EnvironmentError as i32)
            .with_stderr_contains(
                "[..]Could not start migration process to upgrade your Volta directory."
            )
    );
}

#[test]
#[cfg(windows)]
fn v3_to_v4_migrates_symlink_shims() {
    use std::fs;

    let s = sandbox().without_layout_marker().layout_file("v3").build();

    let bin_dir = s.volta_bin_dir();
    let dummy_target = bin_dir.join("volta-shim.exe");
    let symlink_path = bin_dir.join("node.exe");
    let regular_path = bin_dir.join("yarn.exe");

    // Create a dummy target file for the symlink
    fs::write(&dummy_target, b"dummy").unwrap();

    // Remove the default yarn shim symlink (created by sandbox builder)
    // and recreate as a regular file to test that non-symlinks are untouched
    fs::remove_file(&regular_path).unwrap();
    fs::write(&regular_path, b"regular shim").unwrap();

    // Create a symlink shim (should be migrated to .cmd script)
    if std::os::windows::fs::symlink_file(&dummy_target, &symlink_path).is_err() {
        // Skip if we don't have symlink privileges
        return;
    }

    assert!(s.path_exists(".volta/bin/node.exe"));
    assert!(s.path_exists(".volta/bin/yarn.exe"));

    // Run migration
    assert_that!(s.volta("--version"), execs().with_status(0));

    // Symlink shim should be replaced: old .exe removed, .cmd script created
    assert!(!s.path_exists(".volta/bin/node.exe"));
    assert!(s.path_exists(".volta/bin/node.cmd"));

    // Regular file shim should be untouched
    assert!(s.path_exists(".volta/bin/yarn.exe"));
}

#[test]
#[cfg(windows)]
fn v3_to_v4_migrates_symlink_shared_dirs() {
    use std::fs;

    let s = sandbox().without_layout_marker().layout_file("v3").build();

    let shared_root = s.path(".volta/tools/shared");
    let orig_target = s.path(".volta/tools/shared-orig/openssl");
    let symlink_path = shared_root.join("openssl");
    let regular_dir = shared_root.join("zlib");

    // Create the shared lib root and a dummy target directory
    fs::create_dir_all(&orig_target).unwrap();
    fs::write(orig_target.join("libcrypto.a"), b"dummy lib").unwrap();

    // Create a regular (non-symlink) directory with content
    fs::create_dir_all(&regular_dir).unwrap();
    fs::write(regular_dir.join("libz.a"), b"regular lib").unwrap();

    // Create a directory symlink (should be migrated to junction)
    if std::os::windows::fs::symlink_dir(&orig_target, &symlink_path).is_err() {
        // Skip if we don't have symlink privileges
        return;
    }

    assert!(s.path_exists(".volta/tools/shared/openssl"));
    assert!(s.path_exists(".volta/tools/shared/zlib"));

    // Run migration
    assert_that!(s.volta("--version"), execs().with_status(0));

    // Both directories should still exist with contents accessible
    assert!(s.path_exists(".volta/tools/shared/openssl/libcrypto.a"));
    assert!(s.path_exists(".volta/tools/shared/zlib/libz.a"));

    // Clean up junction before sandbox Drop (which can't handle junctions)
    let _ = fs::remove_dir(&symlink_path);
}
