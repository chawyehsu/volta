//! Provides types and functions for fetching and unpacking compressed
//! archives in tarball or zip format.
use std::fs::File;
use std::path::Path;

use attohttpc::header::HeaderMap;
use headers::{ContentLength, Header, HeaderMapExt};
use thiserror::Error;

pub(crate) mod fs_utils;
mod progress_read;
mod tarball;
mod zip;

pub(crate) use tarball::Tarball;
pub(crate) use zip::Zip;

/// Error type for this module
#[derive(Error, Debug)]
pub(crate) enum ArchiveError {
    #[error("HTTP failure ({0})")]
    HttpError(attohttpc::StatusCode),

    #[error("HTTP header '{0}' not found")]
    MissingHeaderError(&'static attohttpc::header::HeaderName),

    #[error("unexpected content length in HTTP response: {0}")]
    UnexpectedContentLengthError(u64),

    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    AttohttpcError(#[from] attohttpc::Error),

    #[error("{0}")]
    ZipError(#[from] zip_rs::result::ZipError),
}

/// Metadata describing whether an archive comes from a local or remote origin.
#[derive(Copy, Clone)]
pub(crate) enum Origin {
    Local,
    Remote,
}

pub(crate) trait Archive {
    fn compressed_size(&self) -> u64;

    /// Unpacks the zip archive to the specified destination folder.
    fn unpack(
        self: Box<Self>,
        dest: &Path,
        progress: &mut dyn FnMut(&(), usize),
    ) -> Result<(), ArchiveError>;

    fn origin(&self) -> Origin;
}

/// Load an archive in the native OS-preferred format from the specified file.
///
/// On Windows, the preferred format is zip. On Unixes, the preferred format
/// is tarball.
#[cfg(unix)]
pub(crate) fn load_native(source: File) -> Result<Box<dyn Archive>, ArchiveError> {
    Tarball::load(source)
}

/// Fetch a remote archive in the native OS-preferred format from the specified
/// URL and store its results at the specified file path.
///
/// On Windows, the preferred format is zip. On Unixes, the preferred format
/// is tarball.
#[cfg(unix)]
pub(crate) fn fetch_native(url: &str, cache_file: &Path) -> Result<Box<dyn Archive>, ArchiveError> {
    Tarball::fetch(url, cache_file)
}

/// Load an archive in the native OS-preferred format from the specified file.
///
/// On Windows, the preferred format is zip. On Unixes, the preferred format
/// is tarball.
#[cfg(windows)]
pub(crate) fn load_native(source: File) -> Result<Box<dyn Archive>, ArchiveError> {
    Zip::load(source)
}

/// Fetch a remote archive in the native OS-preferred format from the specified
/// URL and store its results at the specified file path.
///
/// On Windows, the preferred format is zip. On Unixes, the preferred format
/// is tarball.
#[cfg(windows)]
pub(crate) fn fetch_native(url: &str, cache_file: &Path) -> Result<Box<dyn Archive>, ArchiveError> {
    Zip::fetch(url, cache_file)
}

/// Determines the length of an HTTP response's content in bytes, using
/// the HTTP `"Content-Length"` header.
fn content_length(headers: &HeaderMap) -> Result<u64, ArchiveError> {
    headers
        .typed_get()
        .map(|ContentLength(v)| v)
        .ok_or_else(|| ArchiveError::MissingHeaderError(ContentLength::name()))
}
