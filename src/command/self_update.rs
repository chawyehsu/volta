use crate::command::Command;
use clap::Args;
use self_update::{Release, ReleaseAsset};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use volta_core::{
    error::{ErrorKind, ExitCode, Fallible, VoltaError},
    layout::volta_install,
    session::{ActivityKind, Session},
    sync::VoltaLock,
};

const MAX_CHECKSUM_SIZE: u64 = 4096;
const MAX_ARCHIVE_SIZE: u64 = 100 * 1024 * 1024;
const MAX_EXTRACTED_SIZE: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 1024;

#[derive(Args)]
pub(crate) struct SelfUpdate {}

pub(crate) fn fail(message: impl Into<String>) -> VoltaError {
    ErrorKind::SelfUpdateError {
        reason: message.into(),
    }
    .into()
}

fn sha256_file(path: &std::path::Path) -> Fallible<String> {
    let mut file = File::open(path).map_err(|e| fail(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|e| fail(e.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn parse_checksum(contents: &str) -> Fallible<&str> {
    contents
        .split_whitespace()
        .next()
        .ok_or_else(|| fail("malformed checksum"))
}

/// Selects the archive and checksum assets from a GitHub release based on their names.
fn select_assets<'a>(
    release: &'a Release,
    archive_name: &str,
    checksum_name: &str,
) -> Fallible<(&'a ReleaseAsset, &'a ReleaseAsset)> {
    let archive = release
        .assets()
        .iter()
        .find(|asset| asset.name() == archive_name)
        .ok_or_else(|| fail(format!("release is missing {archive_name}")))?;
    let checksum = release
        .assets()
        .iter()
        .find(|asset| asset.name() == checksum_name)
        .ok_or_else(|| fail(format!("release is missing {checksum_name}")))?;
    Ok((archive, checksum))
}

fn validate_archive_limits(
    archive_path: &Path,
    format: &str,
    max_size: u64,
    max_entries: usize,
) -> Fallible<()> {
    let mut count = 0usize;
    let mut total = 0u64;

    if format == "zip" {
        #[cfg(windows)]
        {
            let file = File::open(archive_path).map_err(|e| fail(e.to_string()))?;
            let mut archive = zip_rs::ZipArchive::new(file).map_err(|e| fail(e.to_string()))?;
            count = archive.len();
            for index in 0..count {
                let entry = archive.by_index(index).map_err(|e| fail(e.to_string()))?;
                total = total.saturating_add(entry.size());
                if count > max_entries || total > max_size {
                    break;
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = archive_path;
            return Err(fail("unsupported archive format"));
        }
    } else {
        let file = File::open(archive_path).map_err(|e| fail(e.to_string()))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        for entry in archive.entries().map_err(|e| fail(e.to_string()))? {
            let entry = entry.map_err(|e| fail(e.to_string()))?;
            count += 1;
            total = total.saturating_add(entry.size());
            if count > max_entries || total > max_size {
                break;
            }
        }
    }

    if count > max_entries {
        return Err(fail(format!(
            "archive contains more than {max_entries} entries"
        )));
    }
    if total > max_size {
        return Err(fail(format!("archive expands beyond {max_size} bytes")));
    }
    Ok(())
}

fn extract_archive(archive_path: &Path, format: &str, destination: &Path) -> Fallible<()> {
    validate_archive_limits(
        archive_path,
        format,
        MAX_EXTRACTED_SIZE,
        MAX_ARCHIVE_ENTRIES,
    )?;
    let archive_kind = if format == "zip" {
        self_update::ArchiveKind::Zip
    } else {
        self_update::ArchiveKind::Tar(Some(self_update::Compression::Gz))
    };
    self_update::Extract::from_source(archive_path)
        .archive(archive_kind)
        .extract_into(destination)
        .map_err(|e| fail(e.to_string()))
}

/// Fetches the latest release from GitHub for the current repository.
///
/// If an API base URL is provided, it will be used instead of the default GitHub API URL.
/// Custom API base URLs are useful for local testing.
fn fetch_latest_release(api_base_url: Option<&str>) -> Fallible<Release> {
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner("chawyehsu")
        .repo_name("volta")
        .bin_name(env!("CARGO_PKG_NAME"))
        .current_version(env!("CARGO_PKG_VERSION"));
    if let Some(url) = api_base_url {
        builder.api_base_url(url);
    }
    builder
        .build()
        .map_err(|e| fail(e.to_string()))?
        .get_latest_release()
        .map_err(|e| fail(e.to_string()))?
        .latest()
        .cloned()
        .ok_or_else(|| fail("GitHub returned no releases"))
}

#[derive(Debug)]
struct StagedBinaries {
    main: PathBuf,
    shim: Option<PathBuf>,
    migrate: Option<PathBuf>,
}

fn staged_path(dir: &Path, name: &str) -> PathBuf {
    #[cfg(windows)]
    {
        dir.join(format!("{name}.exe"))
    }
    #[cfg(not(windows))]
    {
        dir.join(name)
    }
}

fn staged_binaries_dir(dir: &Path) -> PathBuf {
    let nested = dir.join("bin");
    if staged_path(&nested, "volta").is_file() {
        nested
    } else {
        dir.to_path_buf()
    }
}

fn validate_staged_binaries(dir: &Path) -> Fallible<StagedBinaries> {
    let root = staged_binaries_dir(dir);
    let other = if root.as_path() == dir {
        dir.join("bin")
    } else {
        dir.to_path_buf()
    };
    for name in ["volta", "volta-shim", "volta-migrate"] {
        if staged_path(&other, name).exists() {
            return Err(fail(
                "archive contains binaries in both the root and bin directory",
            ));
        }
    }

    let main = staged_path(&root, "volta");
    if !main.is_file() {
        return Err(fail("archive is missing volta"));
    }

    let optional = |name: &str| {
        let path = staged_path(&root, name);
        if path.exists() && !path.is_file() {
            return Err(fail(format!("archive entry {name} is not a file")));
        }
        Ok(path.exists().then_some(path))
    };

    Ok(StagedBinaries {
        main,
        shim: optional("volta-shim")?,
        migrate: optional("volta-migrate")?,
    })
}

fn install_staged(staged: StagedBinaries, install: &Path) -> Fallible<()> {
    #[cfg(windows)]
    crate::utils::cleanup_retired_executables(install);

    let binaries = [
        ("volta", Some(staged.main)),
        ("volta-shim", staged.shim),
        ("volta-migrate", staged.migrate),
    ];

    let backup = tempfile::tempdir_in(install).map_err(|e| fail(e.to_string()))?;
    let originals = binaries
        .iter()
        .map(|(name, _)| {
            let destination = crate::utils::exe_path(&install.join(*name));
            let backup_path = backup.path().join(
                destination
                    .file_name()
                    .ok_or_else(|| fail("install destination has no file name"))?,
            );
            if destination.exists() {
                std::fs::copy(&destination, &backup_path).map_err(|e| fail(e.to_string()))?;
                Ok((*name, Some(backup_path)))
            } else {
                Ok((*name, None))
            }
        })
        .collect::<Fallible<Vec<_>>>()?;

    for (name, source) in &binaries {
        let destination = install.join(*name);
        let result = match source {
            Some(source) => crate::utils::replace_exe(source, &destination),
            None => crate::utils::remove_or_retire(&destination).map(|_| ()),
        };
        if let Err(error) = result {
            return match rollback_install(&originals, install) {
                Ok(()) => Err(error),
                Err(rollback_error) => {
                    let backup_path = backup.keep();
                    Err(fail(format!(
                        "{error}; rollback failed: {rollback_error}; backups retained at {}",
                        backup_path.display()
                    )))
                }
            };
        }
    }
    #[cfg(windows)]
    crate::utils::cleanup_retired_executables(install);
    Ok(())
}

fn rollback_install(originals: &[(&str, Option<PathBuf>)], install: &Path) -> Fallible<()> {
    let mut failures = Vec::new();
    for (name, backup) in originals {
        let destination = install.join(name);
        let result = match backup {
            Some(backup) => crate::utils::replace_exe(backup, &destination),
            None => crate::utils::remove_or_retire(&destination).map(|_| ()),
        };
        if let Err(error) = result {
            failures.push(format!("{name}: {error}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(fail(failures.join("; ")))
    }
}

fn target() -> Option<(&'static str, &'static str)> {
    if cfg!(target_env = "musl") {
        return None;
    }
    Some(match (env::consts::ARCH, env::consts::OS) {
        ("x86_64", "linux") => ("x86_64-unknown-linux-gnu", "tar.gz"),
        ("aarch64", "linux") => ("aarch64-unknown-linux-gnu", "tar.gz"),
        ("x86_64", "macos") => ("x86_64-apple-darwin", "tar.gz"),
        ("aarch64", "macos") => ("aarch64-apple-darwin", "tar.gz"),
        ("x86_64", "windows") => ("x86_64-pc-windows-msvc", "zip"),
        ("aarch64", "windows") => ("aarch64-pc-windows-msvc", "zip"),
        _ => return None,
    })
}

impl Command for SelfUpdate {
    fn run(self, session: &mut Session) -> Fallible<ExitCode> {
        session.add_event_start(ActivityKind::SelfUpdate);
        let result = update();
        match result {
            Ok(()) => {
                session.add_event_end(ActivityKind::SelfUpdate, ExitCode::Success);
                Ok(ExitCode::Success)
            }
            Err(e) => {
                session.add_event_error(ActivityKind::SelfUpdate, &e);
                session.add_event_end(ActivityKind::SelfUpdate, e.exit_code());
                Err(e)
            }
        }
    }
}

fn api_base_url() -> Option<String> {
    #[cfg(feature = "mock-network")]
    if let Some(url) = env::var("VOLTA_MOCK_SERVER_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(url);
    }

    None
}

fn update() -> Fallible<()> {
    let (triplet, format) = target().ok_or_else(|| fail("unsupported target"))?;
    let _lock = VoltaLock::acquire()?;
    let current_version = env!("CARGO_PKG_VERSION");
    let release = fetch_latest_release(api_base_url().as_deref())?;
    let is_greater = self_update::version::bump_is_greater(current_version, release.version())
        .ok()
        .unwrap_or(false);
    if !is_greater && env::var_os("VOLTA_TEST_FORCE_SELFUPDATE").is_none() {
        println!("volta is already up-to-date");
        return Ok(());
    }

    println!(
        "Updating volta: {} -> {}",
        current_version,
        release.version()
    );

    let archive_name = format!("volta-{triplet}.{format}");
    let checksum_name = format!("{archive_name}.sha256");
    let (archive_asset, checksum_asset) = select_assets(&release, &archive_name, &checksum_name)?;
    let temp = tempfile::tempdir().map_err(|e| fail(e.to_string()))?;
    let archive_path = temp.path().join(&archive_name);
    let mut archive_file = File::create(&archive_path).map_err(|e| fail(e.to_string()))?;
    self_update::Download::from_url(archive_asset.download_url())
        .request_header(
            self_update::http::header::ACCEPT,
            "application/octet-stream",
        )
        .max_download_size(MAX_ARCHIVE_SIZE)
        .download_to(&mut archive_file)
        .map_err(|e| fail(e.to_string()))?;

    let mut checksum_file = Vec::new();
    self_update::Download::from_url(checksum_asset.download_url())
        .request_header(
            self_update::http::header::ACCEPT,
            "application/octet-stream",
        )
        .max_download_size(MAX_CHECKSUM_SIZE)
        .download_to(&mut checksum_file)
        .map_err(|e| fail(e.to_string()))?;
    let checksum = String::from_utf8(checksum_file).map_err(|e| fail(e.to_string()))?;
    let expected = parse_checksum(&checksum)?;
    let actual = sha256_file(&archive_path)?;
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(fail("checksum mismatch"));
    }

    let extracted = temp.path().join("extracted");
    std::fs::create_dir(&extracted).map_err(|e| fail(e.to_string()))?;
    extract_archive(&archive_path, format, &extracted)?;
    let install = volta_install()?;

    let staged = validate_staged_binaries(&extracted)?;
    install_staged(staged, install.root())
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "mock-network")]
    use super::fetch_latest_release;
    use super::{
        StagedBinaries, extract_archive, install_staged, parse_checksum, rollback_install,
        select_assets, validate_archive_limits, validate_staged_binaries,
    };
    use self_update::{Release, ReleaseAsset};
    use std::fs;
    use tempfile::TempDir;

    const DIGEST: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn bin(name: &str) -> String {
        format!("{name}{}", std::env::consts::EXE_SUFFIX)
    }

    #[test]
    fn parses_coreutils_checksum() {
        assert_eq!(
            parse_checksum(&format!("{DIGEST}  volta.tar.gz\n")).unwrap(),
            DIGEST
        );
    }

    #[test]
    fn rejects_empty_checksum() {
        assert!(parse_checksum("\n").is_err());
    }

    #[test]
    fn validates_required_and_optional_staged_binaries() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(bin("volta")), b"main").unwrap();
        fs::write(dir.path().join(bin("volta-shim")), b"shim").unwrap();

        let staged = validate_staged_binaries(dir.path()).unwrap();
        assert!(staged.main.is_file());
        assert!(staged.shim.is_some());
        assert!(staged.migrate.is_none());
    }

    #[test]
    fn validates_binaries_in_nested_bin_directory() {
        let dir = TempDir::new().unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir(&bin_dir).unwrap();
        fs::write(bin_dir.join(bin("volta")), b"main").unwrap();
        fs::write(bin_dir.join(bin("volta-shim")), b"shim").unwrap();

        let staged = validate_staged_binaries(dir.path()).unwrap();
        assert_eq!(staged.main, bin_dir.join(bin("volta")));
        assert_eq!(staged.shim, Some(bin_dir.join(bin("volta-shim"))));
        assert!(staged.migrate.is_none());
    }

    #[test]
    fn rejects_archive_with_binaries_in_both_layouts() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(bin("volta-shim")), b"top-level").unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir(&bin_dir).unwrap();
        fs::write(bin_dir.join(bin("volta")), b"nested").unwrap();

        assert!(validate_staged_binaries(dir.path()).is_err());
    }

    #[test]
    fn falls_back_to_top_level_when_nested_bin_has_no_main() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(bin("volta")), b"main").unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir(&bin_dir).unwrap();
        fs::write(bin_dir.join("README"), b"not a binary").unwrap();

        let staged = validate_staged_binaries(dir.path()).unwrap();
        assert_eq!(staged.main, dir.path().join(bin("volta")));
        assert!(staged.shim.is_none());
    }

    #[test]
    fn rejects_staging_without_main_binary() {
        let dir = TempDir::new().unwrap();
        assert!(validate_staged_binaries(dir.path()).is_err());
    }

    #[test]
    fn installs_staged_binaries_and_removes_stale_optional_files() {
        let staged_dir = TempDir::new().unwrap();
        let install_dir = TempDir::new().unwrap();
        fs::write(staged_dir.path().join(bin("volta")), b"new main").unwrap();
        fs::write(staged_dir.path().join(bin("volta-shim")), b"new shim").unwrap();
        fs::write(install_dir.path().join(bin("volta-migrate")), b"stale").unwrap();

        let staged = validate_staged_binaries(staged_dir.path()).unwrap();
        install_staged(staged, install_dir.path()).unwrap();

        assert_eq!(
            fs::read(install_dir.path().join(bin("volta"))).unwrap(),
            b"new main"
        );
        assert_eq!(
            fs::read(install_dir.path().join(bin("volta-shim"))).unwrap(),
            b"new shim"
        );
        assert!(!install_dir.path().join(bin("volta-migrate")).exists());
    }

    #[test]
    fn failed_replacement_keeps_existing_binary() {
        let dir = TempDir::new().unwrap();
        let destination = dir.path().join(bin("volta"));
        fs::write(&destination, b"old main").unwrap();

        let missing = dir.path().join(bin("missing"));
        assert!(crate::utils::replace_exe(&missing, &dir.path().join("volta")).is_err());

        assert_eq!(fs::read(destination).unwrap(), b"old main");
    }

    #[cfg(unix)]
    #[test]
    fn failed_persist_keeps_existing_binary() {
        let dir = TempDir::new().unwrap();
        let destination = dir.path().join(bin("volta"));
        fs::create_dir(&destination).unwrap();
        let new = dir.path().join(bin("new"));
        fs::write(&new, b"new main").unwrap();

        assert!(crate::utils::replace_exe(&new, &dir.path().join("volta")).is_err());

        assert!(destination.is_dir());
    }

    #[test]
    fn restores_previous_binaries_when_a_later_replacement_fails() {
        let staged_dir = TempDir::new().unwrap();
        let install_dir = TempDir::new().unwrap();
        fs::write(staged_dir.path().join(bin("volta")), b"new main").unwrap();
        fs::write(install_dir.path().join(bin("volta")), b"old main").unwrap();
        fs::write(install_dir.path().join(bin("volta-shim")), b"old shim").unwrap();
        fs::write(
            install_dir.path().join(bin("volta-migrate")),
            b"old migrate",
        )
        .unwrap();

        let staged = StagedBinaries {
            main: staged_dir.path().join(bin("volta")),
            shim: Some(staged_dir.path().join(bin("missing"))),
            migrate: None,
        };

        assert!(install_staged(staged, install_dir.path()).is_err());

        assert_eq!(
            fs::read(install_dir.path().join(bin("volta"))).unwrap(),
            b"old main"
        );
        assert_eq!(
            fs::read(install_dir.path().join(bin("volta-shim"))).unwrap(),
            b"old shim"
        );
        assert_eq!(
            fs::read(install_dir.path().join(bin("volta-migrate"))).unwrap(),
            b"old migrate"
        );
    }

    #[test]
    fn rollback_removes_optional_binaries_that_did_not_exist_before() {
        let staged_dir = TempDir::new().unwrap();
        let install_dir = TempDir::new().unwrap();
        fs::write(staged_dir.path().join(bin("volta")), b"new main").unwrap();
        fs::write(install_dir.path().join(bin("volta")), b"old main").unwrap();

        let staged = StagedBinaries {
            main: staged_dir.path().join(bin("volta")),
            shim: Some(staged_dir.path().join(bin("missing"))),
            migrate: None,
        };

        assert!(install_staged(staged, install_dir.path()).is_err());

        assert_eq!(
            fs::read(install_dir.path().join(bin("volta"))).unwrap(),
            b"old main"
        );
        assert!(!install_dir.path().join(bin("volta-shim")).exists());
        assert!(!install_dir.path().join(bin("volta-migrate")).exists());
    }

    #[test]
    fn rollback_attempts_every_restore_after_a_failure() {
        let install_dir = TempDir::new().unwrap();
        let backup_dir = TempDir::new().unwrap();
        let missing_backup = backup_dir.path().join(bin("missing"));
        let valid_backup = backup_dir.path().join(bin("volta-shim"));
        fs::write(&valid_backup, b"old shim").unwrap();
        fs::write(install_dir.path().join(bin("volta-shim")), b"new shim").unwrap();

        let originals = [
            ("volta", Some(missing_backup)),
            ("volta-shim", Some(valid_backup)),
        ];

        assert!(rollback_install(&originals, install_dir.path()).is_err());
        assert_eq!(
            fs::read(install_dir.path().join(bin("volta-shim"))).unwrap(),
            b"old shim"
        );
    }

    #[test]
    fn removing_a_directory_as_an_optional_binary_is_an_error() {
        let dir = TempDir::new().unwrap();
        let destination = dir.path().join(bin("volta-migrate"));
        fs::create_dir(&destination).unwrap();

        assert!(crate::utils::remove_or_retire(&destination).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn cleans_up_retired_executables() {
        let dir = TempDir::new().unwrap();
        let trash = dir.path().join(".trash");
        fs::create_dir(&trash).unwrap();
        let retired = trash.join("volta.exe.old.1.0");
        fs::write(&retired, b"old").unwrap();
        let keep = trash.join("keep");
        fs::create_dir(&keep).unwrap();

        crate::utils::cleanup_retired_executables(dir.path());

        assert!(!retired.exists());
        assert!(keep.exists());
    }

    #[test]
    fn selects_only_the_conventional_archive_and_checksum_assets() {
        let release = Release::builder()
            .version("2.0.4")
            .assets(vec![
                ReleaseAsset::new("notes.txt", "https://example.invalid/notes"),
                ReleaseAsset::new(
                    "volta-x86_64-apple-darwin.tar.gz",
                    "https://example.invalid/archive",
                ),
                ReleaseAsset::new(
                    "volta-x86_64-apple-darwin.tar.gz.sha256",
                    "https://example.invalid/checksum",
                ),
            ])
            .build()
            .unwrap();

        let (archive, checksum) = select_assets(
            &release,
            "volta-x86_64-apple-darwin.tar.gz",
            "volta-x86_64-apple-darwin.tar.gz.sha256",
        )
        .unwrap();
        assert_eq!(archive.download_url(), "https://example.invalid/archive");
        assert_eq!(checksum.download_url(), "https://example.invalid/checksum");
    }

    #[test]
    fn rejects_release_without_checksum_asset() {
        let release = Release::builder()
            .version("2.0.4")
            .assets(vec![ReleaseAsset::new(
                "volta.tar.gz",
                "https://example.invalid/archive",
            )])
            .build()
            .unwrap();
        assert!(select_assets(&release, "volta.tar.gz", "volta.tar.gz.sha256").is_err());
    }

    #[test]
    fn extracts_tar_gz_into_staging_directory() {
        let temp = TempDir::new().unwrap();
        let archive_path = temp.path().join("volta.tar.gz");
        let output = temp.path().join("extracted");
        fs::create_dir(&output).unwrap();

        let archive = fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(archive, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let content = b"binary";
        let mut header = tar::Header::new_gnu();
        header.set_path("volta").unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();
        builder.into_inner().unwrap().finish().unwrap();

        extract_archive(&archive_path, "tar.gz", &output).unwrap();
        assert_eq!(fs::read(output.join("volta")).unwrap(), content);
    }

    #[test]
    fn rejects_archives_that_exceed_extraction_limits() {
        let temp = TempDir::new().unwrap();
        let archive_path = temp.path().join("volta.tar.gz");
        let archive = fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(archive, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let content = b"binary";
        let mut header = tar::Header::new_gnu();
        header.set_path("volta").unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();
        builder.into_inner().unwrap().finish().unwrap();

        assert!(validate_archive_limits(&archive_path, "tar.gz", 0, 10).is_err());
        assert!(validate_archive_limits(&archive_path, "tar.gz", 1024, 0).is_err());
        assert!(validate_archive_limits(&archive_path, "tar.gz", 1024, 10).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_zip_archives_that_exceed_extraction_limits() {
        use std::io::Write;

        let temp = TempDir::new().unwrap();
        let archive_path = temp.path().join("volta.zip");
        let archive = fs::File::create(&archive_path).unwrap();
        let mut writer = zip_rs::ZipWriter::new(archive);
        let options = zip_rs::write::SimpleFileOptions::default();
        writer.start_file("volta.exe", options).unwrap();
        writer.write_all(b"binary").unwrap();
        writer.finish().unwrap();

        assert!(validate_archive_limits(&archive_path, "zip", 0, 10).is_err());
        assert!(validate_archive_limits(&archive_path, "zip", 1024, 0).is_err());
        assert!(validate_archive_limits(&archive_path, "zip", 1024, 10).is_ok());
    }

    #[cfg(feature = "mock-network")]
    #[test]
    fn fetches_latest_release_from_a_local_github_api() {
        let mut server = mockito::Server::new();
        let body = r#"{
            "tag_name": "v2.0.4",
            "name": "Volta 2.0.4",
            "created_at": "2026-01-01T00:00:00Z",
            "published_at": "2026-01-01T00:00:00Z",
            "html_url": "https://example.invalid/releases/v2.0.4",
            "assets": [
                {"name": "volta-x86_64-apple-darwin.tar.gz", "url": "https://example.invalid/api/archive", "browser_download_url": "PLACEHOLDER", "digest": "sha256:abc", "created_at": "2026-01-01T00:00:00Z"},
                {"name": "volta-x86_64-apple-darwin.tar.gz.sha256", "url": "https://example.invalid/api/checksum", "browser_download_url": "PLACEHOLDER", "created_at": "2026-01-01T00:00:00Z"}
            ]
        }"#;
        let mock = server
            .mock("GET", "/repos/chawyehsu/volta/releases/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();

        let release = fetch_latest_release(Some(&server.url())).unwrap();
        mock.assert();
        assert_eq!(release.version(), "2.0.4");
        assert_eq!(release.assets().len(), 2);
        assert_eq!(release.assets()[0].digest(), Some("sha256:abc"));
    }
}
