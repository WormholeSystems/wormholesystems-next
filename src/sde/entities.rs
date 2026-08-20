//! The entity to file to key table: each row wires one top-level SDE struct to its source
//! `.jsonl` file and primary key, which is what lets [`super::load`] / [`super::load_all`]
//! work for every entity.
//!
//! Most files key on an integer `_key` (the `i64` arm); a handful key on a string (the
//! `String` arm, which clones the field).

use super::SdeEntity;

/// Generate `SdeEntity` impls from a `Type => "file.jsonl"` table.
///
/// The default arm is for `i64` keys read straight from `self.id`. Prefix a
/// block with `str:` for entities whose `_key` is a `String` (cloned out).
macro_rules! sde_entities {
    ( $( $ty:ty => $file:literal ),* $(,)? ) => {
        $(
            impl SdeEntity for $ty {
                type Id = i64;
                const FILE: &'static str = $file;
                fn id(&self) -> i64 { self.id }
            }
        )*
    };
    ( str: $( $ty:ty => $file:literal ),* $(,)? ) => {
        $(
            impl SdeEntity for $ty {
                type Id = String;
                const FILE: &'static str = $file;
                fn id(&self) -> String { self.id.clone() }
            }
        )*
    };
}

// ---- integer-keyed entities ----
sde_entities! {
    // character
    super::character::Ancestry           => "ancestries.jsonl",
    super::character::Archetype          => "archetypes.jsonl",
    super::character::Bloodline          => "bloodlines.jsonl",
    super::character::Certificate        => "certificates.jsonl",
    super::character::CharacterAttribute => "characterAttributes.jsonl",
    super::character::CloneGrade         => "cloneGrades.jsonl",
    super::character::Mastery            => "masteries.jsonl",
    super::character::Race               => "races.jsonl",

    // dogma
    super::dogma::DbuffCollection        => "dbuffCollections.jsonl",
    super::dogma::DogmaAttribute         => "dogmaAttributes.jsonl",
    super::dogma::DogmaAttributeCategory => "dogmaAttributeCategories.jsonl",
    super::dogma::DogmaEffect            => "dogmaEffects.jsonl",
    super::dogma::DogmaUnit              => "dogmaUnits.jsonl",

    // inventory
    super::inventory::Blueprint            => "blueprints.jsonl",
    super::inventory::Category             => "categories.jsonl",
    super::inventory::CompressibleType     => "compressibleTypes.jsonl",
    super::inventory::ContrabandType       => "contrabandTypes.jsonl",
    super::inventory::ControlTowerResource => "controlTowerResources.jsonl",
    super::inventory::DynamicItemAttribute => "dynamicItemAttributes.jsonl",
    super::inventory::Group                => "groups.jsonl",
    super::inventory::MarketGroup          => "marketGroups.jsonl",
    super::inventory::MetaGroup            => "metaGroups.jsonl",
    super::inventory::Type                 => "types.jsonl",
    super::inventory::TypeBonus            => "typeBonus.jsonl",
    super::inventory::TypeDogma            => "typeDogma.jsonl",
    super::inventory::TypeElement          => "typeElements.jsonl",
    super::inventory::TypeList             => "typeLists.jsonl",
    super::inventory::TypeMaterials        => "typeMaterials.jsonl",

    // misc
    super::misc::Graphic            => "graphics.jsonl",
    super::misc::GraphicMaterialSet => "graphicMaterialSets.jsonl",
    super::misc::Icon               => "icons.jsonl",
    super::misc::PlanetResource     => "planetResources.jsonl",
    super::misc::PlanetSchematic    => "planetSchematics.jsonl",
    super::misc::ShipTreeElement    => "shipTreeElements.jsonl",
    super::misc::ShipTreeFaction    => "shipTreeFactions.jsonl",
    super::misc::ShipTreeGroup      => "shipTreeGroups.jsonl",

    // npc
    super::npc::AgentInSpace             => "agentsInSpace.jsonl",
    super::npc::AgentType                => "agentTypes.jsonl",
    super::npc::CorporationActivity      => "corporationActivities.jsonl",
    super::npc::Faction                  => "factions.jsonl",
    super::npc::NpcCharacter             => "npcCharacters.jsonl",
    super::npc::NpcCorporation           => "npcCorporations.jsonl",
    super::npc::NpcCorporationDivision   => "npcCorporationDivisions.jsonl",
    super::npc::NpcStation               => "npcStations.jsonl",
    super::npc::StationOperation         => "stationOperations.jsonl",
    super::npc::StationService           => "stationServices.jsonl",

    // pve
    super::pve::Dungeon                     => "dungeons.jsonl",
    super::pve::EpicArc                     => "epicArcs.jsonl",
    super::pve::FreelanceJobSchema          => "freelanceJobSchemas.jsonl",
    super::pve::MercenaryTacticalOperation  => "mercenaryTacticalOperations.jsonl",
    super::pve::Mission                     => "missions.jsonl",
    super::pve::SovereigntyUpgrade          => "sovereigntyUpgrades.jsonl",

    // skin
    super::skin::Skin                     => "skins.jsonl",
    super::skin::SkinLicense              => "skinLicenses.jsonl",
    super::skin::SkinMaterial             => "skinMaterials.jsonl",
    super::skin::SkinrComponent           => "skinrComponents.jsonl",
    super::skin::SkinrComponentCategory   => "skinrComponentCategories.jsonl",
    super::skin::SkinrComponentPointValue => "skinrComponentPointValues.jsonl",
    super::skin::SkinrComponentRarity     => "skinrComponentRarities.jsonl",
    super::skin::SkinrSlot                => "skinrSlots.jsonl",
    super::skin::SkinrSlotCategory       => "skinrSlotCategories.jsonl",
    super::skin::SkinrSlotConfiguration  => "skinrSlotConfigurations.jsonl",
    super::skin::SkinrSlotName           => "skinrSlotNames.jsonl",
    super::skin::SkinrTierThreshold      => "skinrTierThresholds.jsonl",

    // universe
    super::universe::AsteroidBelt  => "mapAsteroidBelts.jsonl",
    super::universe::Constellation => "mapConstellations.jsonl",
    super::universe::Landmark      => "landmarks.jsonl",
    super::universe::Moon          => "mapMoons.jsonl",
    super::universe::Planet        => "mapPlanets.jsonl",
    super::universe::Region        => "mapRegions.jsonl",
    super::universe::SecondarySun  => "mapSecondarySuns.jsonl",
    super::universe::SolarSystem   => "mapSolarSystems.jsonl",
    super::universe::Star          => "mapStars.jsonl",
    super::universe::Stargate      => "mapStargates.jsonl",
}

// ---- string-keyed entities ----
sde_entities! { str:
    super::character::CharacterTitle           => "characterTitles.jsonl",
    super::misc::SdeMeta                        => "_sde.jsonl",
    super::misc::TranslationLanguage           => "translationLanguages.jsonl",
    super::pve::MilitaryCampaign               => "militaryCampaigns.jsonl",
    super::pve::MilitaryCampaignObjective      => "militaryCampaignObjectives.jsonl",
}
