use super::common::LocalizedString;
use serde::Deserialize;

/// `races.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Race {
    #[serde(rename = "_key")]
    pub id: i32,
    pub description: Option<LocalizedString>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<i32>,
    pub name: LocalizedString,
    #[serde(rename = "shipTypeID")]
    pub ship_type_id: Option<i32>,
    pub skills: Option<Vec<RaceSkill>>,
}

/// Map-like `{_key, _value}` pair from `races.jsonl` `skills`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceSkill {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "_value")]
    pub value: i32,
}

/// `bloodlines.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bloodline {
    #[serde(rename = "_key")]
    pub id: i32,
    pub charisma: i32,
    #[serde(rename = "corporationID")]
    pub corporation_id: i32,
    pub description: LocalizedString,
    #[serde(rename = "iconID")]
    pub icon_id: Option<i32>,
    pub intelligence: i32,
    pub memory: i32,
    pub name: LocalizedString,
    pub perception: i32,
    #[serde(rename = "raceID")]
    pub race_id: i32,
    pub willpower: i32,
}

/// `ancestries.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ancestry {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "bloodlineID")]
    pub bloodline_id: i32,
    pub charisma: i32,
    pub description: LocalizedString,
    #[serde(rename = "iconID")]
    pub icon_id: Option<i32>,
    pub intelligence: i32,
    pub memory: i32,
    pub name: LocalizedString,
    pub perception: i32,
    pub short_description: Option<String>,
    pub willpower: i32,
}

/// `characterAttributes.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterAttribute {
    #[serde(rename = "_key")]
    pub id: i32,
    pub description: String,
    #[serde(rename = "iconID")]
    pub icon_id: i32,
    pub name: LocalizedString,
    pub notes: String,
    pub short_description: String,
}

/// `characterTitles.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterTitle {
    #[serde(rename = "_key")]
    pub id: String,
    pub name: LocalizedString,
}

/// `cloneGrades.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneGrade {
    #[serde(rename = "_key")]
    pub id: i32,
    pub name: String,
    pub skills: Vec<CloneGradeSkill>,
}

/// Nested `skills` entry from `cloneGrades.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneGradeSkill {
    pub level: i32,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

/// `archetypes.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Archetype {
    #[serde(rename = "_key")]
    pub id: i32,
    pub description: LocalizedString,
    pub title: Option<LocalizedString>,
}

/// `certificates.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    #[serde(rename = "_key")]
    pub id: i32,
    pub description: LocalizedString,
    #[serde(rename = "groupID")]
    pub group_id: i32,
    pub name: LocalizedString,
    pub recommended_for: Option<Vec<i32>>,
    pub skill_types: Vec<CertificateSkillType>,
}

/// Nested `skillTypes` entry from `certificates.jsonl`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateSkillType {
    #[serde(rename = "_key")]
    pub id: i32,
    pub advanced: i32,
    pub basic: i32,
    pub elite: i32,
    pub improved: i32,
    pub standard: i32,
}

/// `masteries.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mastery {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "_value")]
    pub value: Vec<MasteryValue>,
}

/// Map-like `{_key, _value}` pair from `masteries.jsonl`; `_value` is a list of
/// certificate IDs keyed by mastery level.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteryValue {
    #[serde(rename = "_key")]
    pub id: i32,
    #[serde(rename = "_value")]
    pub value: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sde::load_all;

    #[test]
    fn parses_races() {
        let rows = load_all::<Race>().expect("parse races");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_bloodlines() {
        let rows = load_all::<Bloodline>().expect("parse bloodlines");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_ancestries() {
        let rows = load_all::<Ancestry>().expect("parse ancestries");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_character_attributes() {
        let rows = load_all::<CharacterAttribute>().expect("parse characterAttributes");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_character_titles() {
        let rows = load_all::<CharacterTitle>().expect("parse characterTitles");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_clone_grades() {
        let rows = load_all::<CloneGrade>().expect("parse cloneGrades");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_archetypes() {
        let rows = load_all::<Archetype>().expect("parse archetypes");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_certificates() {
        let rows = load_all::<Certificate>().expect("parse certificates");
        assert!(!rows.is_empty());
    }

    #[test]
    fn parses_masteries() {
        let rows = load_all::<Mastery>().expect("parse masteries");
        assert!(!rows.is_empty());
    }
}
