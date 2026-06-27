use std::{collections::HashMap, fs::File, io::BufRead, io::BufReader, path::Path};

pub mod character;
pub mod common;
pub mod dogma;
pub mod download;
mod entities;
pub mod inventory;
pub mod misc;
pub mod npc;
pub mod pve;
pub mod skin;
pub mod universe;

// Re-export the most commonly used types so callers can write `sde::SolarSystem`.
// Everything else is reachable via its domain module, e.g. `sde::inventory::Type`.
pub use universe::SolarSystem;

/// Directory holding the unpacked SDE `.jsonl` files, relative to the crate
/// root. The downloader writes the archive into `data/` and unpacks it here;
/// hand-authored static JSON that augments the SDE lives alongside in `data/`.
pub const SDE_DIR: &str = "data/sde";

/// An SDE record type backed by one `.jsonl` file and addressable by a primary key.
///
/// Implemented for every top-level type (see `entities.rs`), which is what lets
/// the generic [`load`] / [`load_all`] loaders work for any entity.
pub trait SdeEntity: serde::de::DeserializeOwned {
    /// The primary key type (`i32` for most files, `String` for a few).
    type Id: std::hash::Hash + std::cmp::Eq;
    /// Filename of the backing `.jsonl`, resolved under [`SDE_DIR`].
    const FILE: &'static str;
    /// This record's primary key (the JSON `_key`).
    fn id(&self) -> Self::Id;
}

#[derive(Debug, thiserror::Error)]
pub enum SdeError {
    #[error("could not read SDE file: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not parse SDE row: {0}")]
    Json(#[from] serde_json::Error),
}

/// Loads one `.jsonl` file into a `Vec<T>`, erroring on the first row that doesn't
/// match `T` (so a successful result means every row parsed). Blank lines are skipped.
pub fn load_jsonl<T: serde::de::DeserializeOwned>(
    path: impl AsRef<std::path::Path>,
) -> Result<Vec<T>, SdeError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line)?);
    }

    Ok(rows)
}

/// Load every row of an entity's file into a `Vec`, e.g.
/// `sde::load_all::<inventory::Type>()?`.
pub fn load_all<T: SdeEntity>() -> Result<Vec<T>, SdeError> {
    load_jsonl::<T>(Path::new(SDE_DIR).join(T::FILE))
}

/// Load every row of an entity's file into a `HashMap` keyed by `id`, e.g.
/// `sde::load::<SolarSystem>()?`.
pub fn load<T: SdeEntity>() -> Result<HashMap<T::Id, T>, SdeError> {
    Ok(load_all::<T>()?
        .into_iter()
        .map(|row| (row.id(), row))
        .collect())
}
