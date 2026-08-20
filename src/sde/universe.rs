use super::common::{LocalizedString, Position2D, Position3D};
use serde::Deserialize;

/// `mapRegions.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "constellationIDs")]
    pub constellation_ids: Vec<i32>,
    pub description: Option<LocalizedString>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<i32>,
    pub name: LocalizedString,
    #[serde(rename = "nebulaID")]
    pub nebula_id: i32,
    pub position: Position3D,
    #[serde(rename = "wormholeClassID")]
    pub wormhole_class_id: Option<i32>,
}

/// `mapConstellations.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Constellation {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "factionID")]
    pub faction_id: Option<i32>,
    pub name: LocalizedString,
    pub position: Position3D,
    #[serde(rename = "regionID")]
    pub region_id: i32,
    #[serde(rename = "solarSystemIDs")]
    pub solar_system_ids: Vec<i32>,
    #[serde(rename = "wormholeClassID")]
    pub wormhole_class_id: Option<i32>,
}

/// `mapSolarSystems.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolarSystem {
    #[serde(rename = "_key")]
    pub id: i32,
    pub border: Option<bool>,
    #[serde(rename = "constellationID")]
    pub constellation_id: i32,
    pub hub: Option<bool>,
    pub international: Option<bool>,
    pub luminosity: Option<f64>,
    pub name: LocalizedString,
    #[serde(rename = "planetIDs")]
    pub planet_ids: Option<Vec<i32>>,
    pub position: Position3D,
    #[serde(rename = "position2D")]
    pub position_2d: Option<Position2D>,
    pub radius: f64,
    #[serde(rename = "regionID")]
    pub region_id: i32,
    pub regional: Option<bool>,
    pub security_class: Option<String>,
    pub security_status: f64,
    #[serde(rename = "starID")]
    pub star_id: Option<i32>,
    #[serde(rename = "stargateIDs")]
    pub stargate_ids: Option<Vec<i32>>,
    pub corridor: Option<bool>,
    pub fringe: Option<bool>,
    #[serde(rename = "wormholeClassID")]
    pub wormhole_class_id: Option<i32>,
    pub visual_effect: Option<String>,
    pub disallowed_anchor_categories: Option<Vec<i32>>,
    pub disallowed_anchor_groups: Option<Vec<i32>>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<i32>,
}

/// Nested `attributes` object of `mapPlanets.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetAttributes {
    pub height_map1: i32,
    pub height_map2: i32,
    pub population: bool,
    pub shader_preset: i32,
}

/// Nested `statistics` object shared by planets and moons (includes `pressure`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CelestialStatistics {
    pub density: f64,
    pub eccentricity: f64,
    pub escape_velocity: f64,
    pub locked: bool,
    pub mass_dust: f64,
    pub mass_gas: Option<f64>,
    pub orbit_period: Option<f64>,
    pub orbit_radius: Option<f64>,
    pub pressure: f64,
    pub rotation_rate: f64,
    pub spectral_class: String,
    pub surface_gravity: Option<f64>,
    pub temperature: f64,
}

/// `mapPlanets.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Planet {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "asteroidBeltIDs")]
    pub asteroid_belt_ids: Option<Vec<i32>>,
    pub attributes: PlanetAttributes,
    pub celestial_index: i32,
    #[serde(rename = "moonIDs")]
    pub moon_ids: Option<Vec<i32>>,
    #[serde(rename = "orbitID")]
    pub orbit_id: i32,
    pub position: Position3D,
    pub radius: i32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: i32,
    pub statistics: CelestialStatistics,
    #[serde(rename = "typeID")]
    pub type_id: i32,
    #[serde(rename = "npcStationIDs")]
    pub npc_station_ids: Option<Vec<i32>>,
    pub unique_name: Option<LocalizedString>,
}

/// Nested `attributes` object of `mapMoons.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoonAttributes {
    pub height_map1: i32,
    pub height_map2: i32,
    pub shader_preset: i32,
}

/// `mapMoons.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Moon {
    #[serde(rename = "_key")]
    pub id: i32,
    pub attributes: MoonAttributes,
    pub celestial_index: i32,
    #[serde(rename = "orbitID")]
    pub orbit_id: i32,
    pub orbit_index: i32,
    pub position: Position3D,
    pub radius: f64,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: i32,
    pub statistics: Option<CelestialStatistics>,
    #[serde(rename = "typeID")]
    pub type_id: i32,
    #[serde(rename = "npcStationIDs")]
    pub npc_station_ids: Option<Vec<i32>>,
    pub unique_name: Option<LocalizedString>,
}

/// Nested `statistics` object of `mapStars.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarStatistics {
    pub age: f64,
    pub life: f64,
    pub luminosity: f64,
    pub spectral_class: String,
    pub temperature: f64,
}

/// `mapStars.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Star {
    #[serde(rename = "_key")]
    pub id: i32,
    pub radius: i64,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: i32,
    pub statistics: StarStatistics,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

/// Nested `destination` object of `mapStargates.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StargateDestination {
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: i32,
    #[serde(rename = "stargateID")]
    pub stargate_id: i32,
}

/// `mapStargates.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stargate {
    #[serde(rename = "_key")]
    pub id: i32,
    pub destination: StargateDestination,
    pub position: Position3D,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: i32,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

/// Nested `statistics` object of `mapAsteroidBelts.jsonl` (no `pressure`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeltStatistics {
    pub density: f64,
    pub eccentricity: f64,
    pub escape_velocity: f64,
    pub locked: bool,
    pub mass_dust: f64,
    pub mass_gas: Option<f64>,
    pub orbit_period: f64,
    pub orbit_radius: f64,
    pub rotation_rate: f64,
    pub spectral_class: String,
    pub surface_gravity: f64,
    pub temperature: f64,
}

/// `mapAsteroidBelts.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsteroidBelt {
    #[serde(rename = "_key")]
    pub id: i32,
    pub celestial_index: i32,
    #[serde(rename = "orbitID")]
    pub orbit_id: i32,
    pub orbit_index: i32,
    pub position: Position3D,
    pub radius: Option<f64>,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: i32,
    pub statistics: Option<BeltStatistics>,
    #[serde(rename = "typeID")]
    pub type_id: i32,
    pub unique_name: Option<LocalizedString>,
}

/// `mapSecondarySuns.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondarySun {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "effectBeaconTypeID")]
    pub effect_beacon_type_id: i32,
    pub position: Position3D,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: i32,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

/// `landmarks.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Landmark {
    #[serde(rename = "_key")]
    pub id: i32,
    pub description: LocalizedString,
    pub name: LocalizedString,
    pub position: Position3D,
    #[serde(rename = "iconID")]
    pub icon_id: Option<i32>,
    #[serde(rename = "locationID")]
    pub location_id: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_regions() {
        let Some(rows) = crate::sde::parse_sample::<Region>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_constellations() {
        let Some(rows) = crate::sde::parse_sample::<Constellation>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_solar_systems() {
        let Some(rows) = crate::sde::parse_sample::<SolarSystem>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_planets() {
        let Some(rows) = crate::sde::parse_sample::<Planet>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_moons() {
        let Some(rows) = crate::sde::parse_sample::<Moon>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_stars() {
        let Some(rows) = crate::sde::parse_sample::<Star>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_stargates() {
        let Some(rows) = crate::sde::parse_sample::<Stargate>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_asteroid_belts() {
        let Some(rows) = crate::sde::parse_sample::<AsteroidBelt>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_secondary_suns() {
        let Some(rows) = crate::sde::parse_sample::<SecondarySun>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_landmarks() {
        let Some(rows) = crate::sde::parse_sample::<Landmark>() else {
            return;
        };
        assert!(!rows.is_empty());
    }
}
