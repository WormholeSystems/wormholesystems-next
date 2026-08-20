use super::common::LocalizedString;
use serde::Deserialize;

/// `icons.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
    #[serde(rename = "_key")]
    pub id: i64,
    pub icon_file: String,
}

/// `graphics.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Graphic {
    #[serde(rename = "_key")]
    pub id: i64,
    pub graphic_file: Option<String>,
    pub icon_folder: Option<String>,
    pub sof_faction_name: Option<String>,
    pub sof_hull_name: Option<String>,
    pub sof_race_name: Option<String>,
    #[serde(rename = "sofMaterialSetID")]
    pub sof_material_set_id: Option<i64>,
    pub sof_layout: Option<Vec<String>>,
}

/// Nested color (`a`/`b`/`g`/`r`) used by `graphicMaterialSets.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicMaterialSetColor {
    pub a: f64,
    pub b: f64,
    pub g: f64,
    pub r: f64,
}

/// `graphicMaterialSets.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicMaterialSet {
    #[serde(rename = "_key")]
    pub id: i64,
    pub color_hull: Option<GraphicMaterialSetColor>,
    pub color_primary: Option<GraphicMaterialSetColor>,
    pub color_secondary: Option<GraphicMaterialSetColor>,
    pub color_window: Option<GraphicMaterialSetColor>,
    pub description: String,
    pub sof_faction_name: Option<String>,
    pub sof_race_hint: Option<String>,
    pub material1: Option<String>,
    pub material2: Option<String>,
    pub material3: Option<String>,
    pub material4: Option<String>,
    pub custommaterial1: Option<String>,
    pub custommaterial2: Option<String>,
    pub sof_pattern_name: Option<String>,
    pub res_path_insert: Option<String>,
}

/// `shipTreeElements.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTreeElement {
    #[serde(rename = "_key")]
    pub id: i64,
    pub description: LocalizedString,
    pub icon: String,
    pub name: LocalizedString,
}

/// Nested `{_key, _value}` pair in `shipTreeGroups.jsonl` `elements`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTreeGroupElement {
    #[serde(rename = "_key")]
    pub id: i64,
    #[serde(rename = "_value")]
    pub value: i32,
}

/// Nested skill in `shipTreeGroups.jsonl` `preReqSkills[].skills`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTreeGroupSkill {
    #[serde(rename = "_key")]
    pub id: i64,
    pub display: bool,
    pub level: i32,
}

/// Nested `{_key, skills}` entry in `shipTreeGroups.jsonl` `preReqSkills`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTreeGroupPreReqSkill {
    #[serde(rename = "_key")]
    pub id: i64,
    pub skills: Vec<ShipTreeGroupSkill>,
}

/// `shipTreeGroups.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTreeGroup {
    #[serde(rename = "_key")]
    pub id: i64,
    pub description: Option<LocalizedString>,
    pub elements: Option<Vec<ShipTreeGroupElement>>,
    pub icon: String,
    pub icon_large: String,
    pub icon_small: String,
    #[serde(rename = "iconSmallNPC")]
    pub icon_small_npc: String,
    pub name: LocalizedString,
    pub pre_req_skills: Option<Vec<ShipTreeGroupPreReqSkill>>,
}

/// Nested `{_key, _value}` pair in `shipTreeFactions.jsonl` `elements`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTreeFactionElement {
    #[serde(rename = "_key")]
    pub id: i64,
    #[serde(rename = "_value")]
    pub value: i32,
}

/// `shipTreeFactions.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipTreeFaction {
    #[serde(rename = "_key")]
    pub id: i64,
    pub description: LocalizedString,
    pub elements: Vec<ShipTreeFactionElement>,
    pub icon: String,
}

/// Nested reagent object in `planetResources.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetResourceReagent {
    #[serde(rename = "amount_per_cycle")]
    pub amount_per_cycle: i32,
    #[serde(rename = "cycle_period")]
    pub cycle_period: i32,
    #[serde(rename = "secured_capacity")]
    pub secured_capacity: i32,
    #[serde(rename = "type_id")]
    pub type_id: i64,
    #[serde(rename = "unsecured_capacity")]
    pub unsecured_capacity: i32,
}

/// `planetResources.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetResource {
    #[serde(rename = "_key")]
    pub id: i64,
    pub power: Option<i32>,
    pub workforce: Option<i32>,
    pub reagent: Option<PlanetResourceReagent>,
}

/// Nested `{_key, isInput, quantity}` entry in `planetSchematics.jsonl` `types`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetSchematicType {
    #[serde(rename = "_key")]
    pub id: i64,
    pub is_input: bool,
    pub quantity: i32,
}

/// `planetSchematics.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetSchematic {
    #[serde(rename = "_key")]
    pub id: i64,
    pub cycle_time: i32,
    pub name: LocalizedString,
    pub pins: Vec<i32>,
    pub types: Vec<PlanetSchematicType>,
}

/// `translationLanguages.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationLanguage {
    #[serde(rename = "_key")]
    pub id: String,
    pub name: String,
}

/// `_sde.jsonl`, a single metadata row.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdeMeta {
    #[serde(rename = "_key")]
    pub id: String,
    pub build_number: i32,
    pub release_date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_icons() {
        let Some(rows) = crate::sde::parse_sample::<Icon>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_graphics() {
        let Some(rows) = crate::sde::parse_sample::<Graphic>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_graphic_material_sets() {
        let Some(rows) = crate::sde::parse_sample::<GraphicMaterialSet>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_ship_tree_elements() {
        let Some(rows) = crate::sde::parse_sample::<ShipTreeElement>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_ship_tree_groups() {
        let Some(rows) = crate::sde::parse_sample::<ShipTreeGroup>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_ship_tree_factions() {
        let Some(rows) = crate::sde::parse_sample::<ShipTreeFaction>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_planet_resources() {
        let Some(rows) = crate::sde::parse_sample::<PlanetResource>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_planet_schematics() {
        let Some(rows) = crate::sde::parse_sample::<PlanetSchematic>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_translation_languages() {
        let Some(rows) = crate::sde::parse_sample::<TranslationLanguage>() else {
            return;
        };
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_sde_meta() {
        let Some(rows) = crate::sde::parse_sample::<SdeMeta>() else {
            return;
        };
        assert!(!rows.is_empty());
    }
}
