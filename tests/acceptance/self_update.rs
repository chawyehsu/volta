use std::fs;

use hamcrest2::assert_that;
use hamcrest2::prelude::*;
use test_support::matchers::execs;

use crate::support::sandbox::sandbox;
use crate::support::self_update::{
    FIXTURE_VERSION, FixtureInstall, FixtureRelease, binary_contents,
};
use volta_core::error::ExitCode;

fn installed(install: &FixtureInstall, name: &str) -> Vec<u8> {
    fs::read(install.binary_path(name))
        .unwrap_or_else(|error| panic!("failed to read installed {name}: {error}"))
}

fn snapshot(install: &FixtureInstall) -> Vec<(&'static str, Vec<u8>)> {
    ["volta", "volta-shim", "volta-migrate"]
        .into_iter()
        .map(|name| (name, installed(install, name)))
        .collect()
}

fn assert_untouched(install: &FixtureInstall, before: &[(&'static str, Vec<u8>)]) {
    for (name, contents) in before {
        assert_that!(installed(install, name), eq(contents.clone()));
    }
}

#[test]
fn updates_all_binaries_from_fixture_release() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::valid();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains(format!(
                "Updating volta: {} -> {FIXTURE_VERSION}",
                env!("CARGO_PKG_VERSION")
            ))
    );

    for name in ["volta", "volta-shim", "volta-migrate"] {
        assert_that!(installed(&install, name), eq(binary_contents(name)));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(install.binary_path("volta"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[cfg(windows)]
    {
        let trash = install.path().join(".trash");
        assert!(trash.is_dir(), "expected a .trash directory");
        let retired: Vec<_> = fs::read_dir(&trash)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            retired
                .iter()
                .any(|name| name.starts_with("volta.exe.old.")),
            "expected the running volta.exe to be retired, found {retired:?}"
        );
    }

    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn volta_only_release_removes_optional_binaries() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::volta_only();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();

    assert_that!(
        install.command(&sandbox),
        execs().with_status(ExitCode::Success as i32)
    );

    assert_that!(installed(&install, "volta"), eq(binary_contents("volta")));
    assert!(!install.binary_path("volta-shim").exists());
    assert!(!install.binary_path("volta-migrate").exists());

    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn missing_volta_shim_removes_it_and_updates_the_rest() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::without_optional("volta-shim");
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();

    assert_that!(
        install.command(&sandbox),
        execs().with_status(ExitCode::Success as i32)
    );

    assert_that!(installed(&install, "volta"), eq(binary_contents("volta")));
    assert_that!(
        installed(&install, "volta-migrate"),
        eq(binary_contents("volta-migrate"))
    );
    assert!(!install.binary_path("volta-shim").exists());

    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn missing_volta_migrate_removes_it_and_updates_the_rest() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::without_optional("volta-migrate");
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();

    assert_that!(
        install.command(&sandbox),
        execs().with_status(ExitCode::Success as i32)
    );

    assert_that!(installed(&install, "volta"), eq(binary_contents("volta")));
    assert_that!(
        installed(&install, "volta-shim"),
        eq(binary_contents("volta-shim"))
    );
    assert!(!install.binary_path("volta-migrate").exists());

    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn same_version_reports_up_to_date_and_leaves_installation_untouched() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::up_to_date();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();
    let before = snapshot(&install);

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains("volta is already up-to-date")
    );

    assert_untouched(&install, &before);
    mocks.release.assert();
}

#[test]
fn forced_update_replaces_binaries_when_versions_match() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::up_to_date();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();

    let mut command = install.command(&sandbox);
    command.env("VOLTA_TEST_FORCE_SELFUPDATE", "1");

    assert_that!(
        command,
        execs()
            .with_status(ExitCode::Success as i32)
            .with_stdout_contains(format!(
                "Updating volta: {} -> {}",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_VERSION")
            ))
    );

    for name in ["volta", "volta-shim", "volta-migrate"] {
        assert_that!(installed(&install, name), eq(binary_contents(name)));
    }

    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn checksum_mismatch_leaves_installation_untouched() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::corrupt_checksum();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();
    let before = snapshot(&install);

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::UnknownError as i32)
            .with_stderr_contains("[..]checksum mismatch")
    );

    assert_untouched(&install, &before);
    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn oversized_checksum_is_rejected_without_replacing_binaries() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::oversized_checksum();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();
    let before = snapshot(&install);

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::UnknownError as i32)
            .with_stderr_contains("[..]max_download_size[..]")
    );

    assert_untouched(&install, &before);
    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn missing_archive_fails_without_replacing_binaries() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::without_archive();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();
    let before = snapshot(&install);

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::UnknownError as i32)
            .with_stderr_contains("[..]release is missing[..]")
    );

    assert_untouched(&install, &before);
    mocks.release.assert();
}

#[test]
fn missing_sidecar_fails_without_replacing_binaries() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::without_sidecar();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();
    let before = snapshot(&install);

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::UnknownError as i32)
            .with_stderr_contains("[..]release is missing[..]")
    );

    assert_untouched(&install, &before);
    mocks.release.assert();
}

#[test]
fn missing_required_binary_fails_without_replacing_binaries() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::without_binary("volta");
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();
    let before = snapshot(&install);

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::UnknownError as i32)
            .with_stderr_contains("[..]archive is missing volta")
    );

    assert_untouched(&install, &before);
    mocks.release.assert();
    mocks.archive.as_ref().unwrap().assert();
    mocks.checksum.as_ref().unwrap().assert();
}

#[test]
fn legacy_named_release_is_rejected_without_replacing_binaries() {
    let mut sandbox = sandbox().build();
    let fixture = FixtureRelease::legacy_named();
    let mocks = fixture.serve(&mut sandbox);
    let install = FixtureInstall::new();
    let before = snapshot(&install);

    assert_that!(
        install.command(&sandbox),
        execs()
            .with_status(ExitCode::UnknownError as i32)
            .with_stderr_contains("[..]release is missing[..]")
    );

    assert_untouched(&install, &before);
    mocks.release.assert();
}
