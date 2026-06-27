//! Fetching the SDE from CCP's static-data endpoint.
//!
//! This is the *acquisition* half of the module: it talks to the network and
//! knows nothing about how the `.jsonl` files are parsed (see the loaders in
//! [`super`]). Hand the downloaded archive to [`crate::util::archive::extract`]
//! to unpack it onto disk before loading.

use std::io;

use reqwest::blocking::Client;
use serde::Deserialize;

const LATEST_BUILD_URL: &str =
    "https://developers.eveonline.com/static-data/tranquility/latest.jsonl";

const DATA_URL: &str =
    "https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-{}-jsonl.zip";

/// Anything that can go wrong while fetching the SDE.
#[derive(Debug)]
pub enum DownloadError {
    /// The HTTP request itself failed.
    Http(reqwest::Error),
    /// The `latest.jsonl` response didn't match [`LatestBuild`].
    Parse(serde_json::Error),
    /// Writing the downloaded archive to disk failed.
    Io(io::Error),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Http(e) => write!(f, "SDE download request failed: {e}"),
            DownloadError::Parse(e) => write!(f, "could not parse latest-build response: {e}"),
            DownloadError::Io(e) => write!(f, "could not write SDE archive: {e}"),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        DownloadError::Http(e)
    }
}
impl From<serde_json::Error> for DownloadError {
    fn from(e: serde_json::Error) -> Self {
        DownloadError::Parse(e)
    }
}
impl From<io::Error> for DownloadError {
    fn from(e: io::Error) -> Self {
        DownloadError::Io(e)
    }
}

/// Metadata about the most recent SDE build, from `latest.jsonl`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestBuild {
    #[serde(rename = "_key")]
    pub key: String,
    pub build_number: i64,
    pub release_date: String,
}

/// Downloads SDE builds from CCP's static-data endpoint.
pub struct Downloader {
    client: Client,
}

impl Downloader {
    pub fn new() -> Downloader {
        Downloader {
            client: Client::new(),
        }
    }

    /// Look up which build is currently the latest.
    pub fn latest_build(&self) -> Result<LatestBuild, DownloadError> {
        let text = self.client.get(LATEST_BUILD_URL).send()?.text()?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Download a specific build's archive and write it to `dest`.
    ///
    /// The body is streamed straight to the file rather than buffered in
    /// memory — the archive is ~100 MB.
    pub fn download_build(
        &self,
        build: i64,
        dest: impl AsRef<std::path::Path>,
    ) -> Result<(), DownloadError> {
        let url = DATA_URL.replace("{}", &build.to_string());
        let mut response = self.client.get(url).send()?;
        let mut file = std::fs::File::create(dest)?;
        response.copy_to(&mut file)?;
        Ok(())
    }

    /// Download whichever build is currently latest, writing it to `dest`.
    pub fn download_latest(&self, dest: impl AsRef<std::path::Path>) -> Result<(), DownloadError> {
        let latest = self.latest_build()?;
        self.download_build(latest.build_number, dest)
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}
