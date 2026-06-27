//! Seeds the reference tables from the SDE (`data/sde/`) and the custom static data
//! (`data/static/`). Run with `cargo run -- seed` (or the built binary + `seed`).
//!
//! Everything runs in one transaction: the entity-cycle FKs are deferred, so factions
//! and corporations (which reference each other) validate at commit. Inline FKs are
//! satisfied by insert order.

use std::collections::HashMap;

use serde::Deserialize;
use sqlx::{PgPool, Postgres, QueryBuilder};

use crate::sde::common::LocalizedString;
use crate::sde::{inventory, npc, universe};

type BoxError = Box<dyn std::error::Error>;

fn en(name: &LocalizedString) -> String {
    name.en.clone().unwrap_or_default()
}

/// Chunked multi-row insert with `on conflict do nothing`, returning the row count.
macro_rules! bulk {
    ($tx:expr, $table_cols:literal, $rows:expr, |$b:ident, $r:pat_param| $body:expr) => {{
        let rows = $rows;
        for chunk in rows.chunks(1000) {
            let mut qb = QueryBuilder::<Postgres>::new(concat!("insert into ", $table_cols, " "));
            qb.push_values(chunk, |mut $b, $r| {
                $body;
            });
            qb.push(" on conflict do nothing");
            qb.build().execute(&mut *$tx).await?;
        }
        rows.len()
    }};
}

pub async fn run() -> Result<(), BoxError> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")?;
    let pool = crate::db::connect(&url).await?;
    seed_all(&pool).await
}

async fn seed_all(pool: &PgPool) -> Result<(), BoxError> {
    // Load the SDE rows we need (the loaders read data/sde/*.jsonl).
    let categories = crate::sde::load_all::<inventory::Category>()?;
    let groups = crate::sde::load_all::<inventory::Group>()?;
    let market_groups = crate::sde::load_all::<inventory::MarketGroup>()?;
    let types = crate::sde::load_all::<inventory::Type>()?;
    let factions = crate::sde::load_all::<npc::Faction>()?;
    let corporations = crate::sde::load_all::<npc::NpcCorporation>()?;
    let regions = crate::sde::load_all::<universe::Region>()?;
    let constellations = crate::sde::load_all::<universe::Constellation>()?;
    let solar_systems = crate::sde::load_all::<universe::SolarSystem>()?;

    let mut tx = pool.begin().await?;

    let n = bulk!(tx, "categories (id, name, published)", &categories, |b, c| {
        b.push_bind(c.id as i64).push_bind(en(&c.name)).push_bind(c.published)
    });
    println!("categories: {n}");

    let n = bulk!(tx, "groups (id, category_id, name, published)", &groups, |b, g| {
        b.push_bind(g.id as i64)
            .push_bind(g.category_id as i64)
            .push_bind(en(&g.name))
            .push_bind(g.published)
    });
    println!("groups: {n}");

    let n = bulk!(tx, "market_groups (id, parent_group_id, name, has_types)", &market_groups, |b, m| {
        b.push_bind(m.id as i64)
            .push_bind(m.parent_group_id.map(|v| v as i64))
            .push_bind(en(&m.name))
            .push_bind(m.has_types)
    });
    println!("market_groups: {n}");

    let n = bulk!(tx, "types (id, group_id, market_group_id, name, published, volume, mass, capacity, icon_id)", &types, |b, t| {
        b.push_bind(t.id as i64)
            .push_bind(t.group_id as i64)
            .push_bind(t.market_group_id.map(|v| v as i64))
            .push_bind(en(&t.name))
            .push_bind(t.published)
            .push_bind(t.volume)
            .push_bind(t.mass)
            .push_bind(t.capacity)
            .push_bind(t.icon_id.map(|v| v as i64))
    });
    println!("types: {n}");

    // factions <-> corporations form a deferred cycle; both go in this transaction.
    let n = bulk!(tx, "factions (id, name, description, corporation_id, militia_corporation_id, home_solar_system_id, size_factor)", &factions, |b, f| {
        b.push_bind(f.id as i64)
            .push_bind(en(&f.name))
            .push_bind(en(&f.description))
            .push_bind(f.corporation_id.map(|v| v as i64))
            .push_bind(f.militia_corporation_id.map(|v| v as i64))
            .push_bind(f.solar_system_id as i64)
            .push_bind(f.size_factor)
    });
    println!("factions: {n}");

    let n = bulk!(tx, "corporations (id, name, ticker, faction_id, ceo_id)", &corporations, |b, c| {
        b.push_bind(c.id as i64)
            .push_bind(en(&c.name))
            .push_bind(&c.ticker_name)
            .push_bind(c.faction_id.map(|v| v as i64))
            .push_bind(c.ceo_id.map(|v| v as i64))
    });
    println!("corporations: {n}");

    let n = bulk!(tx, "regions (id, name, faction_id, wormhole_class_id)", &regions, |b, r| {
        b.push_bind(r.id as i64)
            .push_bind(en(&r.name))
            .push_bind(r.faction_id.map(|v| v as i64))
            .push_bind(r.wormhole_class_id)
    });
    println!("regions: {n}");

    let n = bulk!(tx, "constellations (id, region_id, name, faction_id)", &constellations, |b, c| {
        b.push_bind(c.id as i64)
            .push_bind(c.region_id as i64)
            .push_bind(en(&c.name))
            .push_bind(c.faction_id.map(|v| v as i64))
    });
    println!("constellations: {n}");

    let n = bulk!(tx, "solar_systems (id, constellation_id, region_id, name, security_status, security_class, faction_id, wormhole_class_id, star_id)", &solar_systems, |b, s| {
        b.push_bind(s.id as i64)
            .push_bind(s.constellation_id as i64)
            .push_bind(s.region_id as i64)
            .push_bind(en(&s.name))
            .push_bind(s.security_status)
            .push_bind(s.security_class.clone())
            .push_bind(s.faction_id.map(|v| v as i64))
            .push_bind(s.wormhole_class_id)
            .push_bind(s.star_id.map(|v| v as i64))
    });
    println!("solar_systems: {n}");

    seed_static(&mut tx, &solar_systems).await?;

    tx.commit().await?;
    println!("seeding complete.");
    Ok(())
}

// ---- custom static data (data/static/*.json) ----

#[derive(Deserialize)]
struct WormholesFile {
    wormholes: HashMap<String, WhType>,
}
#[derive(Deserialize)]
struct WhType {
    #[serde(rename = "typeID")]
    type_id: i64,
    dest: Option<i32>,
    #[serde(default)]
    src: Vec<i32>,
    #[serde(rename = "static")]
    is_static: Option<bool>,
    max_mass_per_jump: Option<i64>,
    total_mass: Option<i64>,
    mass_regen: Option<i64>,
    lifetime: Option<f64>,
    sibling_groups: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct WhSystemsFile {
    systems: Vec<WhSystem>,
}
#[derive(Deserialize)]
struct WhSystem {
    id: i64,
    class: Option<i32>,
    effect: Option<String>,
    #[serde(default)]
    statics: Vec<String>,
}

#[derive(Deserialize)]
struct SignaturesFile {
    categories: Vec<SigCategory>,
    types: Vec<SigType>,
}
#[derive(Deserialize)]
struct SigCategory {
    id: i64,
    name: String,
    code: String,
}
#[derive(Deserialize)]
struct SigType {
    id: i64,
    name: String,
    signature: Option<String>,
    signature_category_id: i64,
    target_class: Option<i32>,
    extra: Option<String>,
    #[serde(default)]
    spawn_areas: Vec<i32>,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, BoxError> {
    Ok(serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?)
}

async fn seed_static(
    tx: &mut sqlx::PgConnection,
    solar_systems: &[universe::SolarSystem],
) -> Result<(), BoxError> {
    // wormhole effects + modifiers
    let effects: HashMap<String, HashMap<String, HashMap<String, Vec<String>>>> =
        read_json("data/static/wormhole_effects.json")?;
    let effect_names: Vec<String> = effects.keys().cloned().collect();
    let mut modifiers = Vec::new();
    for (effect, kinds) in &effects {
        for (kind_name, stats) in kinds {
            let kind = if kind_name == "Buffs" { "buff" } else { "debuff" };
            for (stat, values) in stats {
                for (i, value) in values.iter().enumerate() {
                    modifiers.push((effect.clone(), kind, stat.clone(), (i + 1) as i32, value.clone()));
                }
            }
        }
    }
    let n = bulk!(tx, "wormhole_effects (name)", &effect_names, |b, name| { b.push_bind(name) });
    println!("wormhole_effects: {n}");
    let n = bulk!(tx, "wormhole_effect_modifiers (effect_name, kind, stat, wormhole_class_id, value)", &modifiers, |b, m| {
        b.push_bind(&m.0).push_bind(m.1).push_bind(&m.2).push_bind(m.3).push_bind(&m.4)
    });
    println!("wormhole_effect_modifiers: {n}");

    // wormhole types + sources
    let wh: WormholesFile = read_json("data/static/wormholes.json")?;
    let wh_types: Vec<(String, WhType)> = wh.wormholes.into_iter().collect();
    let mut sources = Vec::new();
    for (code, t) in &wh_types {
        for src in &t.src {
            sources.push((code.clone(), *src));
        }
    }
    let n = bulk!(tx, "wormhole_types (code, type_id, dest_class, is_static, max_mass_per_jump, total_mass, mass_regen, lifetime_hours, sibling_groups)", &wh_types, |b, (code, t)| {
        b.push_bind(code)
            .push_bind(t.type_id)
            .push_bind(t.dest)
            .push_bind(t.is_static)
            .push_bind(t.max_mass_per_jump)
            .push_bind(t.total_mass)
            .push_bind(t.mass_regen)
            .push_bind(t.lifetime)
            .push_bind(t.sibling_groups.clone())
    });
    println!("wormhole_types: {n}");
    let n = bulk!(tx, "wormhole_type_sources (wormhole_code, wormhole_class_id)", &sources, |b, s| {
        b.push_bind(&s.0).push_bind(s.1)
    });
    println!("wormhole_type_sources: {n}");

    // wormhole systems + statics (need a real class; skip rows without one)
    let whs: WhSystemsFile = read_json("data/static/wormhole_systems.json")?;
    let wh_systems: Vec<WhSystem> = whs.systems.into_iter().filter(|s| s.class.is_some()).collect();
    let mut statics = Vec::new();
    for s in &wh_systems {
        for code in &s.statics {
            statics.push((s.id, code.clone()));
        }
    }
    let n = bulk!(tx, "wormhole_systems (solar_system_id, wormhole_class_id, effect_name)", &wh_systems, |b, s| {
        b.push_bind(s.id).push_bind(s.class.unwrap()).push_bind(s.effect.clone())
    });
    println!("wormhole_systems: {n}");
    let n = bulk!(tx, "wormhole_system_statics (solar_system_id, wormhole_code)", &statics, |b, s| {
        b.push_bind(s.0).push_bind(&s.1)
    });
    println!("wormhole_system_statics: {n}");

    // signature catalogue
    let sig: SignaturesFile = read_json("data/static/signatures.json")?;
    let mut spawn_areas = Vec::new();
    for t in &sig.types {
        for area in &t.spawn_areas {
            spawn_areas.push((t.id, *area));
        }
    }
    let n = bulk!(tx, "signature_categories (id, name, code)", &sig.categories, |b, c| {
        b.push_bind(c.id).push_bind(&c.name).push_bind(&c.code)
    });
    println!("signature_categories: {n}");
    let n = bulk!(tx, "signature_types (id, signature, name, signature_category_id, target_class, extra)", &sig.types, |b, t| {
        b.push_bind(t.id)
            .push_bind(&t.signature)
            .push_bind(&t.name)
            .push_bind(t.signature_category_id)
            .push_bind(t.target_class)
            .push_bind(t.extra.clone())
    });
    println!("signature_types: {n}");
    let n = bulk!(tx, "signature_type_spawn_areas (signature_type_id, wormhole_class_id)", &spawn_areas, |b, s| {
        b.push_bind(s.0).push_bind(s.1)
    });
    println!("signature_type_spawn_areas: {n}");

    // jove observatories: source is region -> [system names]; resolve names to ids.
    let by_name: HashMap<String, i64> =
        solar_systems.iter().map(|s| (en(&s.name), s.id as i64)).collect();
    let jove: HashMap<String, Vec<String>> = read_json("data/static/jove_observatories.json")?;
    let mut jove_ids: Vec<i64> = jove
        .values()
        .flatten()
        .filter_map(|name| by_name.get(name).copied())
        .collect();
    jove_ids.sort_unstable();
    jove_ids.dedup();
    let n = bulk!(tx, "jove_observatories (solar_system_id)", &jove_ids, |b, id| { b.push_bind(*id) });
    println!("jove_observatories: {n}");

    Ok(())
}
