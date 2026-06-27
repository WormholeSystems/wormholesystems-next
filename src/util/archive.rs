//! Unpacking `.zip` archives onto disk.
//!
//! Generic over any zip — it has no SDE knowledge; it's used to unpack a
//! downloaded SDE build before the loaders read it. Fails fast: a successful
//! result means every entry was written.

use std::{fs, io, path::Path};

use zip::{ZipArchive, result::ZipError};

/// Anything that can go wrong while extracting an archive.
#[derive(Debug)]
pub enum ExtractError {
    /// Opening or reading the zip container failed.
    Zip(ZipError),
    /// Writing an extracted entry to disk failed.
    Io(io::Error),
    /// An entry's path escaped the output directory (a "zip slip" attempt).
    UnsafePath,
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::Zip(e) => write!(f, "could not read zip archive: {e}"),
            ExtractError::Io(e) => write!(f, "could not write extracted file: {e}"),
            ExtractError::UnsafePath => write!(f, "archive entry has an unsafe path"),
        }
    }
}

impl std::error::Error for ExtractError {}

impl From<ZipError> for ExtractError {
    fn from(e: ZipError) -> Self {
        ExtractError::Zip(e)
    }
}
impl From<io::Error> for ExtractError {
    fn from(e: io::Error) -> Self {
        ExtractError::Io(e)
    }
}

/// Extract every entry of `archive` into `output_dir`, creating directories as
/// needed. Stops at the first failure.
pub fn extract(
    archive: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> Result<(), ExtractError> {
    let output_dir = output_dir.as_ref();
    let mut archive = ZipArchive::new(fs::File::open(archive)?)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;

        // `enclosed_name` returns `None` for paths that would escape the output
        // directory, so a missing name means the entry is unsafe to extract.
        let out_path = output_dir.join(entry.enclosed_name().ok_or(ExtractError::UnsafePath)?);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        io::copy(&mut entry, &mut fs::File::create(&out_path)?)?;
    }

    Ok(())
}
