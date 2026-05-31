use crate::support::temp_project::temp_project;
use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

const PACKAGE_JSON: &str = r#"
{
    "name": "my-library",
    "version": "1.0.0",
    "bin": {
        "mylibrary": "./index.js"
    }
}"#;

const INDEX_JS: &str = r#"#!/usr/bin/env node

console.log('VOLTA TEST');
"#;

#[test]
fn link_global_command_unsupported() {
    let p = temp_project()
        .package_json(PACKAGE_JSON)
        .project_file("index.js", INDEX_JS)
        .env("VOLTA_FEATURE_PNPM", "1")
        .build();

    // Install node and pnpm to ensure pnpm is available
    assert_that!(
        p.volta("install node@16.15.1 pnpm@7.7.1"),
        execs().with_status(0)
    );

    // Volta currently blocks pnpm global commands.
    assert_that!(
        p.pnpm("link --global"),
        execs()
            .with_status(126)
            .with_stderr_contains("[..]pnpm global commands is not supported yet.")
    );

    // Global package link should not be created.
    assert!(!p.shim_exists("mylibrary"));
    assert!(!p.package_is_installed("my-library"));
}
