use super::common::LocalizedString;
use serde::Deserialize;

/// `skins.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "allowCCPDevs")]
    pub allow_ccp_devs: bool,
    pub internal_name: String,
    #[serde(rename = "skinMaterialID")]
    pub skin_material_id: i32,
    pub types: Vec<i32>,
    pub visible_serenity: bool,
    pub visible_tranquility: bool,
    pub is_structure_skin: Option<bool>,
}

/// `skinMaterials.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinMaterial {
    #[serde(rename = "_key")]
    pub id: i32,
    pub display_name: Option<LocalizedString>,
    #[serde(rename = "materialSetID")]
    pub material_set_id: i32,
}

/// `skinLicenses.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinLicense {
    #[serde(rename = "_key")]
    pub id: i32,
    pub duration: i32,
    #[serde(rename = "licenseTypeID")]
    pub license_type_id: i32,
    #[serde(rename = "skinID")]
    pub skin_id: i32,
    pub is_single_use: Option<bool>,
}

/// Element of `skinrComponents.associatedTypeIds`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrComponentAssociatedTypeId {
    pub license_uses_granted: i32,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

/// `skinrComponents.sequenceBinder`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrComponentSequenceBinder {
    pub count: i32,
    #[serde(rename = "itemTypeID")]
    pub item_type_id: i32,
}

/// `skinrComponents.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrComponent {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "associatedTypeIds")]
    pub associated_type_ids: Vec<SkinrComponentAssociatedTypeId>,
    pub category: i32,
    pub finish: String,
    pub icon_file: String,
    pub name: LocalizedString,
    pub projection_type_u: String,
    pub projection_type_v: String,
    pub published: bool,
    pub rarity: i32,
    pub resource_file: String,
    pub sequence_binder: SkinrComponentSequenceBinder,
}

/// `skinrComponentCategories.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrComponentCategory {
    #[serde(rename = "_key")]
    pub id: i32,
    pub name: String,
}

/// `skinrComponentRarities.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrComponentRarity {
    #[serde(rename = "_key")]
    pub id: i32,
    pub name: LocalizedString,
    pub rank: i32,
}

/// Element of `skinrComponentPointValues._value`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrComponentPointValueEntry {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "_value")]
    pub value: i32,
}

/// `skinrComponentPointValues.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrComponentPointValue {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "_value")]
    pub value: Vec<SkinrComponentPointValueEntry>,
}

/// `skinrSlots.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrSlot {
    #[serde(rename = "_key")]
    pub id: i32,
    pub allowed_design_component_categories: Vec<i32>,
    pub category: i32,
    pub name: LocalizedString,
}

/// `skinrSlotCategories.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrSlotCategory {
    #[serde(rename = "_key")]
    pub id: i32,
    pub name: String,
}

/// `skinrSlotNames.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrSlotName {
    #[serde(rename = "_key")]
    pub id: i32,
    pub name: String,
}

/// `skinrSlotConfigurations.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrSlotConfiguration {
    #[serde(rename = "_key")]
    pub id: i32,
    pub allow_all_ships: Option<bool>,
    pub config: Option<Vec<i32>>,
    pub name: String,
    pub priority: i32,
    pub ships: Option<Vec<i32>>,
}

/// Element of `skinrTierThresholds._value`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrTierThresholdEntry {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "_value")]
    pub value: i32,
}

/// `skinrTierThresholds.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinrTierThreshold {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "_value")]
    pub value: Vec<SkinrTierThresholdEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sde::load_all;

    #[test]
    fn parses_skins() {
        let rows = load_all::<Skin>().expect("parse skins");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skin_materials() {
        let rows = load_all::<SkinMaterial>().expect("parse skinMaterials");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skin_licenses() {
        let rows = load_all::<SkinLicense>().expect("parse skinLicenses");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_components() {
        let rows = load_all::<SkinrComponent>().expect("parse skinrComponents");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_component_categories() {
        let rows = load_all::<SkinrComponentCategory>().expect("parse skinrComponentCategories");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_component_rarities() {
        let rows = load_all::<SkinrComponentRarity>().expect("parse skinrComponentRarities");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_component_point_values() {
        let rows = load_all::<SkinrComponentPointValue>().expect("parse skinrComponentPointValues");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_slots() {
        let rows = load_all::<SkinrSlot>().expect("parse skinrSlots");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_slot_categories() {
        let rows = load_all::<SkinrSlotCategory>().expect("parse skinrSlotCategories");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_slot_names() {
        let rows = load_all::<SkinrSlotName>().expect("parse skinrSlotNames");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_slot_configurations() {
        let rows = load_all::<SkinrSlotConfiguration>().expect("parse skinrSlotConfigurations");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_skinr_tier_thresholds() {
        let rows = load_all::<SkinrTierThreshold>().expect("parse skinrTierThresholds");
        assert!(!rows.is_empty());
    }
}
