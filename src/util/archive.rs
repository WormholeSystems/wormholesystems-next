//! Unpacking `.zip` archives onto disk.
//!
//! Generic over any zip — it has no SDE knowledge; it's used to unpack a
//! downloaded SDE build before the loaders read it. Fails fast: a successful
//! result means every entry was written.

use std::{fs, io, path::Path};

use zip::{ZipArchive, result::ZipError};

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("could not read zip archive: {0}")]
    Zip(#[from] ZipError),
    #[error("could not write extracted file: {0}")]
    Io(#[from] io::Error),
    /// An entry's path escaped the output directory (a "zip slip" attempt).
    #[error("archive entry has an unsafe path")]
    UnsafePath,
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
