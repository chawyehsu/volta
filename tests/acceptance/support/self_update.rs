use std::env;
use std::fs;
use std::path::PathBuf;

#[cfg(windows)]
use std::path::Path;

use mockito::Mock;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use test_support::process::ProcessBuilder;

use super::sandbox::{Sandbox, cargo_dir};

const TRIPLETS: [(&str, &str); 6] = [
    ("x86_64-unknown-linux-gnu", "tar.gz"),
    ("aarch64-unknown-linux-gnu", "tar.gz"),
    ("x86_64-apple-darwin", "tar.gz"),
    ("aarch64-apple-darwin", "tar.gz"),
    ("x86_64-pc-windows-msvc", "zip"),
    ("aarch64-pc-windows-msvc", "zip"),
];

pub const FIXTURE_VERSION: &str = "9.9.9";

fn host_target() -> (&'static str, &'static str) {
    match (env::consts::ARCH, env::consts::OS) {
        ("x86_64", "linux") => ("x86_64-unknown-linux-gnu", "tar.gz"),
        ("aarch64", "linux") => ("aarch64-unknown-linux-gnu", "tar.gz"),
        ("x86_64", "macos") => ("x86_64-apple-darwin", "tar.gz"),
        ("aarch64", "macos") => ("aarch64-apple-darwin", "tar.gz"),
        ("x86_64", "windows") => ("x86_64-pc-windows-msvc", "zip"),
        ("aarch64", "windows") => ("aarch64-pc-windows-msvc", "zip"),
        _ => panic!("unsupported host target"),
    }
}

fn legacy_os_name() -> &'static str {
    match env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "win",
        _ => "unknown",
    }
}

fn binary_filename(name: &str) -> String {
    format!("{name}{}", env::consts::EXE_SUFFIX)
}

pub fn binary_contents(name: &str) -> Vec<u8> {
    format!("fixture {name} {FIXTURE_VERSION}\n").into_bytes()
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(not(windows))]
fn archive_bytes(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    use flate2::write::GzEncoder;

    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    {
        let mut builder = tar::Builder::new(&mut encoder);
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(name).unwrap();
            header.set_size(content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append(&header, content.as_slice()).unwrap();
        }
        builder.finish().unwrap();
    }
    encoder.finish().unwrap()
}

#[cfg(windows)]
fn archive_bytes(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    use std::io::{Cursor, Write};

    use zip_rs::write::SimpleFileOptions;

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = zip_rs::ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().unix_permissions(0o755);
        for (name, content) in entries {
            writer.start_file(name.as_str(), options).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap();
    }
    cursor.into_inner()
}

#[derive(Clone, Copy)]
enum ChecksumMode {
    Valid,
    Corrupt,
    Oversized,
}

pub struct FixtureRelease {
    version: &'static str,
    binaries: Vec<&'static str>,
    checksum_mode: ChecksumMode,
    include_archive: bool,
    include_checksum: bool,
    legacy_names: bool,
    nested_bin: bool,
}

pub struct FixtureMocks {
    pub release: Mock,
    pub archive: Option<Mock>,
    pub checksum: Option<Mock>,
}

impl FixtureRelease {
    pub fn valid() -> Self {
        Self {
            version: FIXTURE_VERSION,
            binaries: vec!["volta", "volta-shim", "volta-migrate"],
            checksum_mode: ChecksumMode::Valid,
            include_archive: true,
            include_checksum: true,
            legacy_names: false,
            nested_bin: false,
        }
    }

    pub fn up_to_date() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            ..Self::valid()
        }
    }

    pub fn volta_only() -> Self {
        Self {
            binaries: vec!["volta"],
            ..Self::valid()
        }
    }

    pub fn without_optional(name: &'static str) -> Self {
        assert!(name == "volta-shim" || name == "volta-migrate");
        let mut release = Self::valid();
        release.binaries.retain(|binary| *binary != name);
        release
    }

    pub fn without_binary(name: &'static str) -> Self {
        let mut release = Self::valid();
        release.binaries.retain(|binary| *binary != name);
        release
    }

    pub fn corrupt_checksum() -> Self {
        Self {
            checksum_mode: ChecksumMode::Corrupt,
            ..Self::valid()
        }
    }

    pub fn oversized_checksum() -> Self {
        Self {
            checksum_mode: ChecksumMode::Oversized,
            ..Self::valid()
        }
    }

    pub fn without_archive() -> Self {
        Self {
            include_archive: false,
            ..Self::valid()
        }
    }

    pub fn without_sidecar() -> Self {
        Self {
            include_checksum: false,
            ..Self::valid()
        }
    }

    pub fn legacy_named() -> Self {
        Self {
            legacy_names: true,
            ..Self::valid()
        }
    }

    pub fn nested_bin() -> Self {
        Self {
            nested_bin: true,
            ..Self::valid()
        }
    }

    pub fn serve(&self, sandbox: &mut Sandbox) -> FixtureMocks {
        let server_url = sandbox.server().url();
        let (host_triplet, host_format) = host_target();
        let archive_name = if self.legacy_names {
            format!("volta-{}-{}.tar.gz", self.version, legacy_os_name())
        } else {
            format!("volta-{host_triplet}.{host_format}")
        };
        let checksum_name = format!("{archive_name}.sha256");

        let entries: Vec<(String, Vec<u8>)> = self
            .binaries
            .iter()
            .map(|name| {
                let path = if self.nested_bin {
                    format!("bin/{}", binary_filename(name))
                } else {
                    binary_filename(name)
                };
                (path, binary_contents(name))
            })
            .collect();
        let archive = self.include_archive.then(|| archive_bytes(&entries));
        let checksum = self.include_checksum.then(|| match self.checksum_mode {
            ChecksumMode::Valid => format!(
                "{}  {archive_name}\n",
                sha256(archive.as_deref().unwrap_or_default())
            )
            .into_bytes(),
            ChecksumMode::Corrupt => format!("{}  {archive_name}\n", "0".repeat(64)).into_bytes(),
            ChecksumMode::Oversized => vec![b'0'; 5000],
        });

        let asset_names = if self.legacy_names {
            vec![archive_name.clone(), checksum_name.clone()]
        } else {
            let mut names = Vec::new();
            for (triplet, format) in TRIPLETS {
                let archive = format!("volta-{triplet}.{format}");
                let checksum = format!("{archive}.sha256");
                let is_host = triplet == host_triplet;
                if !is_host || self.include_archive {
                    names.push(archive);
                }
                if !is_host || self.include_checksum {
                    names.push(checksum);
                }
            }
            names
        };

        let assets: Vec<_> = asset_names
            .iter()
            .map(|name| {
                json!({
                    "name": name,
                    "url": format!("{server_url}/assets/{name}"),
                })
            })
            .collect();
        let release_body = json!({
            "tag_name": format!("v{}", self.version),
            "name": format!("Volta {}", self.version),
            "created_at": "2026-01-01T00:00:00Z",
            "assets": assets,
        })
        .to_string();

        let release = sandbox
            .server()
            .mock("GET", "/repos/chawyehsu/volta/releases/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(release_body)
            .create();

        let archive = archive.map(|bytes| {
            let path = format!("/assets/{archive_name}");
            sandbox
                .server()
                .mock("GET", path.as_str())
                .with_status(200)
                .with_body(bytes)
                .create()
        });
        let checksum = checksum.map(|bytes| {
            let path = format!("/assets/{checksum_name}");
            sandbox
                .server()
                .mock("GET", path.as_str())
                .with_status(200)
                .with_body(bytes)
                .create()
        });

        FixtureMocks {
            release,
            archive,
            checksum,
        }
    }
}

pub struct FixtureInstall {
    dir: TempDir,
}

impl FixtureInstall {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create fixture install directory");
        for name in ["volta", "volta-shim", "volta-migrate"] {
            let source = cargo_dir().join(binary_filename(name));
            let destination = dir.path().join(binary_filename(name));
            fs::copy(&source, &destination).unwrap_or_else(|error| {
                panic!(
                    "failed to copy {} to {}: {}",
                    source.display(),
                    destination.display(),
                    error
                )
            });
        }
        Self { dir }
    }

    #[cfg(windows)]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn binary_path(&self, name: &str) -> PathBuf {
        self.dir.path().join(binary_filename(name))
    }

    pub fn command(&self, sandbox: &Sandbox) -> ProcessBuilder {
        let mut process = sandbox.process(self.binary_path("volta"));
        process.arg("self-update");
        process
    }
}
