use serde::Deserialize;

/// A string localized into the languages the SDE ships.
///
/// Most rows carry all eight languages, but some legacy rows ship only a
/// subset (often just `en`), so every field is optional.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LocalizedString {
    pub de: Option<String>,
    pub en: Option<String>,
    pub es: Option<String>,
    pub fr: Option<String>,
    pub ja: Option<String>,
    pub ko: Option<String>,
    pub ru: Option<String>,
    pub zh: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Position3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Position2D {
    pub x: f64,
    pub y: f64,
}
