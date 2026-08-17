//! Killmail ingest + threat analysis — the legacy rules, ported.
//!
//! Ingest: poll zKillboard's R2Z2 sequence stream and persist a **minimal** row per
//! killmail (id, hash, system, time, participating orgs). The full ESI payload is not
//! kept; threat analysis only needs who was involved where and when.
//!
//! Analysis (daily): per wormhole system over the last 90 days, count kills per
//! organisation (victim + attackers, alliance preferred over corporation, each org at
//! most once per killmail), keep orgs active on >= 5 distinct days, take the top 10 by
//! kills; the summed kills decide the threat level: >= 50 critical, >= 15 high, else
//! unknown.
//!
//! Both loops only run when `ZKB_LISTEN=1` (dev machines shouldn't hammer zKillboard).

use std::time::Duration;

use serde::Deserialize;
use sqlx::PgPool;

use crate::entities::EntityKind;
use crate::esi::EsiClient;

const DEFAULT_BASE: &str = "https://r2z2.zkillboard.com";
const RETENTION_DAYS: i32 = 730;
const ANALYSIS_WINDOW_DAYS: i32 = 90;
const MIN_ACTIVE_DAYS: i64 = 5;
const TOP_ORGS: i64 = 10;
const HOSTILE_THRESHOLD: i64 = 50;
/// How far back the killmails card looks, and therefore how far back names are resolved.
pub const CARD_WINDOW_DAYS: i32 = 7;
const ACTIVE_THRESHOLD: i64 = 15;

/// The compact per-killmail org record persisted in `killmails.orgs`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct Org {
    pub id: i64,
    pub kind: String,
}

#[derive(Deserialize)]
struct R2Z2Sequence {
    sequence: i64,
}

#[derive(Deserialize)]
struct R2Z2Killmail {
    killmail_id: i64,
    hash: String,
    esi: serde_json::Value,
    /// zKillboard's own summary: value, attacker count, and the solo/NPC flags. Absent on
    /// a malformed frame, which is why every field below is read defensively.
    #[serde(default)]
    zkb: serde_json::Value,
}

/// The parts of a killmail a reader cares about, pulled out of the two payloads.
struct Detail {
    victim_character_id: Option<i64>,
    victim_corporation_id: Option<i64>,
    victim_alliance_id: Option<i64>,
    victim_ship_type_id: Option<i64>,
    total_value: Option<f64>,
    attacker_count: Option<i32>,
    is_npc: bool,
    is_solo: bool,
    final_blow_character_id: Option<i64>,
    final_blow_corporation_id: Option<i64>,
    final_blow_alliance_id: Option<i64>,
    final_blow_ship_type_id: Option<i64>,
}

fn extract_detail(esi: &serde_json::Value, zkb: &serde_json::Value) -> Detail {
    let victim = &esi["victim"];
    // The killing blow is the one attacker worth naming; the rest are a count.
    let final_blow = esi["attackers"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["final_blow"].as_bool() == Some(true)));
    let id = |v: &serde_json::Value, key: &str| v[key].as_i64();
    Detail {
        victim_character_id: id(victim, "character_id"),
        victim_corporation_id: id(victim, "corporation_id"),
        victim_alliance_id: id(victim, "alliance_id"),
        victim_ship_type_id: id(victim, "ship_type_id"),
        total_value: zkb["totalValue"].as_f64(),
        attacker_count: zkb["attackerCount"]
            .as_i64()
            .or_else(|| esi["attackers"].as_array().map(|a| a.len() as i64))
            .map(|n| n as i32),
        is_npc: zkb["npc"].as_bool().unwrap_or(false),
        is_solo: zkb["solo"].as_bool().unwrap_or(false),
        final_blow_character_id: final_blow.and_then(|a| id(a, "character_id")),
        final_blow_corporation_id: final_blow.and_then(|a| id(a, "corporation_id")),
        final_blow_alliance_id: final_blow.and_then(|a| id(a, "alliance_id")),
        final_blow_ship_type_id: final_blow.and_then(|a| id(a, "ship_type_id")),
    }
}

/// zKillboard and EVE Ref reject anonymous clients (403), so identify ourselves.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!(
            "vector-wormhole-mapper/",
            env!("CARGO_PKG_VERSION"),
            " (tim.kunze4@gmail.com)"
        ))
        .build()
        .expect("http client")
}

/// Spawn the ingest + analysis loops (gated by `ZKB_LISTEN=1`).
pub fn start(pool: PgPool, esi: EsiClient, maps: crate::maps::MapHub) {
    if std::env::var("ZKB_LISTEN").as_deref() != Ok("1") {
        return;
    }
    let base = std::env::var("ZKB_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    tokio::spawn(listen(pool.clone(), base, maps));
    tokio::spawn(resolve_loop(pool.clone(), esi.clone()));
    tokio::spawn(analysis_loop(pool, esi));
}

async fn listen(pool: PgPool, base: String, maps: crate::maps::MapHub) {
    let http = http_client();
    loop {
        match ingest_next(&pool, &http, &base, &maps).await {
            Ok(true) => tokio::time::sleep(Duration::from_millis(500)).await,
            Ok(false) => tokio::time::sleep(Duration::from_secs(10)).await,
            Err(err) => {
                eprintln!("killmail ingest error: {err}");
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }
}

/// One entity as a killmail row names it: a portrait, and something to call them.
#[derive(Debug, Clone, serde::Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct KillParty {
    #[ts(optional)]
    pub character_id: Option<i64>,
    #[ts(optional)]
    pub character_name: Option<String>,
    #[ts(optional)]
    pub corporation_id: Option<i64>,
    #[ts(optional)]
    pub corporation_ticker: Option<String>,
    #[ts(optional)]
    pub alliance_id: Option<i64>,
    #[ts(optional)]
    pub alliance_ticker: Option<String>,
    #[ts(optional)]
    pub ship_type_id: Option<i64>,
    #[ts(optional)]
    pub ship_name: Option<String>,
}

/// A killmail as the card shows it.
///
/// Only what a row renders, rather than the raw payload: the ESI body carries every
/// attacker and every destroyed item, which for fifty kills is orders of magnitude more
/// than the handful of fields on screen.
#[derive(Debug, Clone, serde::Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapKillmail {
    pub id: i64,
    pub solar_system_id: i64,
    pub system_name: String,
    pub region: String,
    pub security_status: f64,
    #[ts(optional)]
    pub wormhole_class_id: Option<i64>,
    pub time: chrono::DateTime<chrono::Utc>,
    pub victim: KillParty,
    pub final_blow: KillParty,
    #[ts(optional)]
    pub total_value: Option<f64>,
    pub attacker_count: i32,
    pub is_npc: bool,
    pub is_solo: bool,
}

/// Which half of the chain a map's killmail card is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillmailFilter {
    All,
    Wormhole,
    KnownSpace,
}

impl KillmailFilter {
    pub fn from_db(value: &str) -> KillmailFilter {
        match value {
            "jspace" => KillmailFilter::Wormhole,
            "kspace" => KillmailFilter::KnownSpace,
            _ => KillmailFilter::All,
        }
    }
}

/// Recent kills in the systems currently on a map, newest first.
///
/// Bounded by time as well as by count, which legacy is not: with only a row cap, a quiet
/// chain shows kills from a year ago as though they were news.
pub async fn list_for_map(
    pool: &PgPool,
    map_id: i64,
    filter: KillmailFilter,
    limit: i64,
) -> sqlx::Result<Vec<MapKillmail>> {
    let wormholes_only = matches!(filter, KillmailFilter::Wormhole);
    let kspace_only = matches!(filter, KillmailFilter::KnownSpace);
    let rows = sqlx::query!(
        r#"select k.id, k.solar_system_id, k.time, k.total_value,
                  coalesce(k.attacker_count, 0) as "attacker_count!",
                  k.is_npc, k.is_solo,
                  ss.name as system_name, r.name as region, ss.security_status,
                  ws.wormhole_class_id as "wormhole_class_id?",
                  k.victim_character_id, vc.name as "victim_character_name?",
                  k.victim_corporation_id, vco.ticker as "victim_corporation_ticker?",
                  k.victim_alliance_id, va.ticker as "victim_alliance_ticker?",
                  k.victim_ship_type_id, vt.name as "victim_ship_name?",
                  k.final_blow_character_id, fc.name as "final_blow_character_name?",
                  k.final_blow_corporation_id, fco.ticker as "final_blow_corporation_ticker?",
                  k.final_blow_alliance_id, fa.ticker as "final_blow_alliance_ticker?",
                  k.final_blow_ship_type_id, ft.name as "final_blow_ship_name?"
           from killmails k
           join map_solar_systems mss
               on mss.map_id = $1 and mss.solar_system_id = k.solar_system_id
           join solar_systems ss on ss.id = k.solar_system_id
           join constellations c on c.id = ss.constellation_id
           join regions r on r.id = c.region_id
           left join wormhole_systems ws on ws.solar_system_id = ss.id
           left join eve_characters vc on vc.id = k.victim_character_id
           left join corporations vco on vco.id = k.victim_corporation_id
           left join alliances va on va.id = k.victim_alliance_id
           left join types vt on vt.id = k.victim_ship_type_id
           left join eve_characters fc on fc.id = k.final_blow_character_id
           left join corporations fco on fco.id = k.final_blow_corporation_id
           left join alliances fa on fa.id = k.final_blow_alliance_id
           left join types ft on ft.id = k.final_blow_ship_type_id
           where k.time >= now() - make_interval(days => $2)
             -- Rows from before the ingest kept any detail would render as blank lines.
             and k.victim_ship_type_id is not null
             and (not $3 or ws.solar_system_id is not null)
             and (not $4 or ws.solar_system_id is null)
           order by k.time desc
           limit $5"#,
        map_id,
        CARD_WINDOW_DAYS,
        wormholes_only,
        kspace_only,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| MapKillmail {
            id: r.id,
            solar_system_id: r.solar_system_id,
            system_name: r.system_name,
            region: r.region,
            security_status: r.security_status,
            wormhole_class_id: r.wormhole_class_id.map(i64::from),
            time: r.time,
            victim: KillParty {
                character_id: r.victim_character_id,
                character_name: r.victim_character_name,
                corporation_id: r.victim_corporation_id,
                corporation_ticker: r.victim_corporation_ticker,
                alliance_id: r.victim_alliance_id,
                alliance_ticker: r.victim_alliance_ticker,
                ship_type_id: r.victim_ship_type_id,
                ship_name: r.victim_ship_name,
            },
            final_blow: KillParty {
                character_id: r.final_blow_character_id,
                character_name: r.final_blow_character_name,
                corporation_id: r.final_blow_corporation_id,
                corporation_ticker: r.final_blow_corporation_ticker,
                alliance_id: r.final_blow_alliance_id,
                alliance_ticker: r.final_blow_alliance_ticker,
                ship_type_id: r.final_blow_ship_type_id,
                ship_name: r.final_blow_ship_name,
            },
            total_value: r.total_value,
            attacker_count: r.attacker_count,
            is_npc: r.is_npc,
            is_solo: r.is_solo,
        })
        .collect())
}

/// Put names to the ids on recent killmails.
///
/// Deliberately a separate loop rather than part of the ingest: a killmail must be
/// recorded whether or not ESI is answering, and the names are only needed by the time
/// someone looks at the card. Runs often enough that a fresh kill is named within a
/// minute or two of arriving.
async fn resolve_loop(pool: PgPool, esi: EsiClient) {
    loop {
        resolve_recent(&pool, &esi).await;
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn resolve_recent(pool: &PgPool, esi: &EsiClient) {
    // The window matches what the card can show, so we never fetch a name nobody will
    // read. `distinct` because a busy system is the same few corps over and over.
    let Ok(rows) = sqlx::query!(
        r#"select distinct victim_character_id, victim_corporation_id, victim_alliance_id,
                  final_blow_character_id, final_blow_corporation_id, final_blow_alliance_id
           from killmails
           where time >= now() - make_interval(days => $1)"#,
        CARD_WINDOW_DAYS,
    )
    .fetch_all(pool)
    .await
    else {
        return;
    };

    let mut characters = Vec::new();
    let mut corporations = Vec::new();
    let mut alliances = Vec::new();
    for row in rows {
        characters.extend(row.victim_character_id);
        characters.extend(row.final_blow_character_id);
        corporations.extend(row.victim_corporation_id);
        corporations.extend(row.final_blow_corporation_id);
        alliances.extend(row.victim_alliance_id);
        alliances.extend(row.final_blow_alliance_id);
    }
    // Organisations first: they name the most rows per fetch, so if the run is cut short
    // by a rate limit the card still reads better than it did.
    crate::entities::ensure(pool, esi, EntityKind::Alliance, &alliances).await;
    crate::entities::ensure(pool, esi, EntityKind::Corporation, &corporations).await;
    crate::entities::ensure(pool, esi, EntityKind::Character, &characters).await;
}

/// Fetch and persist the next killmail in the sequence. Returns whether one was found.
async fn ingest_next(
    pool: &PgPool,
    http: &reqwest::Client,
    base: &str,
    maps: &crate::maps::MapHub,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let cursor: Option<i64> = sqlx::query_scalar("select sequence_id from zkb_state")
        .fetch_optional(pool)
        .await?;
    let next = match cursor {
        Some(seq) => seq + 1,
        None => {
            // First run: start at the live head of the stream.
            let head: R2Z2Sequence = http
                .get(format!("{base}/ephemeral/sequence.json"))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            sqlx::query("insert into zkb_state (id, sequence_id) values (true, $1)")
                .bind(head.sequence)
                .execute(pool)
                .await?;
            head.sequence + 1
        }
    };

    let res = http
        .get(format!("{base}/ephemeral/{next}.json"))
        .send()
        .await?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(false); // caught up
    }
    let body = res.error_for_status()?.text().await?;
    let Ok(km) = serde_json::from_str::<R2Z2Killmail>(&body) else {
        // Empty/garbage frame: advance past it.
        advance(pool, next).await?;
        return Ok(true);
    };

    let solar_system_id = km.esi["solar_system_id"].as_i64().unwrap_or(0);
    let time = km.esi["killmail_time"].as_str().unwrap_or_default();
    let orgs = extract_orgs(&km.esi);
    let detail = extract_detail(&km.esi, &km.zkb);
    if solar_system_id != 0 && !time.is_empty() {
        // The retention check happens in SQL (chrono's clock is disabled in this crate).
        sqlx::query(
            "insert into killmails (
                 id, hash, solar_system_id, time, orgs,
                 victim_character_id, victim_corporation_id, victim_alliance_id,
                 victim_ship_type_id, total_value, attacker_count, is_npc, is_solo,
                 final_blow_character_id, final_blow_corporation_id,
                 final_blow_alliance_id, final_blow_ship_type_id
             )
             select $1, $2, $3, $4::timestamptz, $5, $7, $8, $9, $10, $11, $12, $13, $14,
                    $15, $16, $17, $18
             where $4::timestamptz >= now() - make_interval(days => $6)
             on conflict (id) do nothing",
        )
        .bind(km.killmail_id)
        .bind(&km.hash)
        .bind(solar_system_id)
        .bind(time)
        .bind(serde_json::to_value(&orgs)?)
        .bind(RETENTION_DAYS)
        .bind(detail.victim_character_id)
        .bind(detail.victim_corporation_id)
        .bind(detail.victim_alliance_id)
        .bind(detail.victim_ship_type_id)
        .bind(detail.total_value)
        .bind(detail.attacker_count)
        .bind(detail.is_npc)
        .bind(detail.is_solo)
        .bind(detail.final_blow_character_id)
        .bind(detail.final_blow_corporation_id)
        .bind(detail.final_blow_alliance_id)
        .bind(detail.final_blow_ship_type_id)
        .execute(pool)
        .await?;
        announce(pool, maps, solar_system_id).await;
    }
    advance(pool, next).await?;
    Ok(true)
}

/// Tell every map holding this system that something died in it.
///
/// The event carries no payload: what a client should show depends on its own filter and
/// its own list, so it refetches rather than trying to splice one row in.
async fn announce(pool: &PgPool, maps: &crate::maps::MapHub, solar_system_id: i64) {
    let Ok(map_ids) = sqlx::query_scalar!(
        "select distinct map_id from map_solar_systems where solar_system_id = $1",
        solar_system_id,
    )
    .fetch_all(pool)
    .await
    else {
        return;
    };
    for map_id in map_ids {
        maps.publish(crate::maps::MapEvent::KillmailReceived { map_id });
    }
}

async fn advance(pool: &PgPool, seq: i64) -> Result<(), sqlx::Error> {
    sqlx::query("update zkb_state set sequence_id = $1")
        .bind(seq)
        .execute(pool)
        .await?;
    Ok(())
}

/// The organisations participating in a killmail (victim + every attacker), alliance
/// preferred over corporation, each org at most once.
pub fn extract_orgs(esi: &serde_json::Value) -> Vec<Org> {
    let mut seen = std::collections::HashSet::new();
    let mut orgs = Vec::new();
    let push = |entity: &serde_json::Value,
                seen: &mut std::collections::HashSet<(i64, bool)>,
                orgs: &mut Vec<Org>| {
        let (id, alliance) = match entity["alliance_id"].as_i64() {
            Some(id) => (id, true),
            None => match entity["corporation_id"].as_i64() {
                Some(id) => (id, false),
                None => return,
            },
        };
        if seen.insert((id, alliance)) {
            orgs.push(Org {
                id,
                kind: if alliance { "alliance" } else { "corporation" }.to_string(),
            });
        }
    };
    push(&esi["victim"], &mut seen, &mut orgs);
    for attacker in esi["attackers"].as_array().unwrap_or(&Vec::new()) {
        push(attacker, &mut seen, &mut orgs);
    }
    orgs
}

/// The legacy thresholds: summed top-org kills decide the level.
pub fn threat_level(total_kills: i64) -> &'static str {
    if total_kills >= HOSTILE_THRESHOLD {
        "critical"
    } else if total_kills >= ACTIVE_THRESHOLD {
        "high"
    } else {
        "unknown"
    }
}

async fn analysis_loop(pool: PgPool, esi: EsiClient) {
    loop {
        if let Err(err) = analyze(&pool, &esi).await {
            eprintln!("threat analysis failed: {err}");
        }
        if let Err(err) = purge(&pool).await {
            eprintln!("killmail purge failed: {err}");
        }
        tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await;
    }
}

struct OrgStat {
    solar_system_id: i64,
    entity_type: String,
    entity_id: i64,
    kills: i64,
}

/// Recompute threat for every wormhole system (full replacement).
pub async fn analyze(pool: &PgPool, esi: &EsiClient) -> Result<(), Box<dyn std::error::Error>> {
    let rows = sqlx::query(
        r#"with orgs as (
               select k.solar_system_id, (o->>'id')::bigint as entity_id,
                      o->>'kind' as entity_type, k.id as killmail_id, date(k.time) as day
               from killmails k
               cross join lateral jsonb_array_elements(k.orgs) o
               where k.time >= now() - make_interval(days => $1)
                 and k.solar_system_id in (select solar_system_id from wormhole_systems)
           ),
           stats as (
               select solar_system_id, entity_type, entity_id,
                      count(distinct killmail_id) as kills, count(distinct day) as active_days
               from orgs group by 1, 2, 3
           ),
           ranked as (
               select *, row_number() over (
                   partition by solar_system_id order by kills desc, entity_id
               ) as rn
               from stats where active_days >= $2
           )
           select solar_system_id, entity_type, entity_id, kills::bigint as kills
           from ranked where rn <= $3"#,
    )
    .bind(ANALYSIS_WINDOW_DAYS)
    .bind(MIN_ACTIVE_DAYS)
    .bind(TOP_ORGS)
    .fetch_all(pool)
    .await?;

    use sqlx::Row;
    let stats: Vec<OrgStat> = rows
        .into_iter()
        .map(|r| OrgStat {
            solar_system_id: r.get("solar_system_id"),
            entity_type: r.get("entity_type"),
            entity_id: r.get("entity_id"),
            kills: r.get("kills"),
        })
        .collect();

    // Resolve entity names: local tables first, ESI for the rest (best effort).
    let mut names: std::collections::HashMap<(String, i64), String> =
        std::collections::HashMap::new();
    for s in &stats {
        let key = (s.entity_type.clone(), s.entity_id);
        if names.contains_key(&key) {
            continue;
        }
        let local: Option<String> = if s.entity_type == "alliance" {
            sqlx::query_scalar("select name from alliances where id = $1")
                .bind(s.entity_id)
                .fetch_optional(pool)
                .await?
        } else {
            sqlx::query_scalar("select name from corporations where id = $1")
                .bind(s.entity_id)
                .fetch_optional(pool)
                .await?
        };
        let name = match local {
            Some(name) => name,
            None => {
                let fetched = if s.entity_type == "alliance" {
                    esi.alliance(s.entity_id).await.map(|a| a.name).ok()
                } else {
                    esi.corporation(s.entity_id).await.map(|c| c.name).ok()
                };
                fetched.unwrap_or_else(|| "Unknown entity".to_string())
            }
        };
        names.insert(key, name);
    }

    // Full replacement in one transaction.
    let mut tx = pool.begin().await?;
    sqlx::query("delete from wormhole_system_threats")
        .execute(&mut *tx)
        .await?;
    sqlx::query("update wormhole_systems set threat_level = 'unknown', threat_analyzed_at = now()")
        .execute(&mut *tx)
        .await?;

    let mut totals: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for s in &stats {
        *totals.entry(s.solar_system_id).or_default() += s.kills;
        let name = &names[&(s.entity_type.clone(), s.entity_id)];
        sqlx::query(
            "insert into wormhole_system_threats (solar_system_id, entity_id, entity_type, name, kills)
             values ($1, $2, $3, $4, $5)",
        )
        .bind(s.solar_system_id)
        .bind(s.entity_id)
        .bind(&s.entity_type)
        .bind(name)
        .bind(s.kills as i32)
        .execute(&mut *tx)
        .await?;
    }
    for (system, total) in &totals {
        sqlx::query("update wormhole_systems set threat_level = $2 where solar_system_id = $1")
            .bind(system)
            .bind(threat_level(*total))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn purge(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("delete from killmails where time < now() - make_interval(days => $1)")
        .bind(RETENTION_DAYS)
        .execute(pool)
        .await?;
    Ok(())
}

/// Backfill killmails from EVE Ref's daily archives (`vector killmails-backfill <days>`),
/// most recent day first. Each day is one `killmails-YYYY-MM-DD.tar.bz2` download,
/// extracted with the system `tar` and bulk-inserted (existing ids untouched, so the
/// live listener's rows are kept). Ends with a threat analysis run so the data shows up
/// immediately.
pub async fn backfill(pool: &PgPool, esi: &EsiClient, days: u32) -> Result<(), BoxError> {
    let http = http_client();
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let scratch = std::env::temp_dir().join("vector-killmails");
    std::fs::create_dir_all(&scratch)?;

    for offset in 1..=i64::from(days) {
        let day = chrono::DateTime::from_timestamp(now_secs - offset * 86_400, 0)
            .expect("valid timestamp")
            .date_naive();
        let name = format!("killmails-{}.tar.bz2", day.format("%Y-%m-%d"));
        let url = format!(
            "https://data.everef.net/killmails/{}/{name}",
            day.format("%Y")
        );

        print!("{day}: downloading… ");
        let res = http.get(&url).send().await?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            println!("no archive");
            continue;
        }
        let bytes = res.error_for_status()?.bytes().await?;
        let archive = scratch.join(&name);
        std::fs::write(&archive, &bytes)?;

        let extract_dir = scratch.join(day.format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&extract_dir)?;
        let status = tokio::process::Command::new("tar")
            .arg("-xjf")
            .arg(&archive)
            .arg("-C")
            .arg(&extract_dir)
            .status()
            .await?;
        if !status.success() {
            return Err(format!("tar failed for {name}").into());
        }

        let mut files = Vec::new();
        collect_json_files(&extract_dir, &mut files)?;
        let mut inserted = 0usize;
        for chunk in files.chunks(500) {
            let mut ids = Vec::new();
            let mut hashes = Vec::new();
            let mut systems = Vec::new();
            let mut times = Vec::new();
            let mut orgs_json = Vec::new();
            for path in chunk {
                let Ok(text) = std::fs::read_to_string(path) else {
                    continue;
                };
                let Ok(km) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let (Some(id), Some(system), Some(time)) = (
                    km["killmail_id"].as_i64(),
                    km["solar_system_id"].as_i64(),
                    km["killmail_time"].as_str(),
                ) else {
                    continue;
                };
                ids.push(id);
                hashes.push(km["killmail_hash"].as_str().unwrap_or_default().to_string());
                systems.push(system);
                times.push(time.to_string());
                orgs_json.push(serde_json::to_value(extract_orgs(&km))?);
            }
            let n = sqlx::query(
                "insert into killmails (id, hash, solar_system_id, time, orgs)
                 select * from unnest($1::bigint[], $2::text[], $3::bigint[],
                                      $4::text[]::timestamptz[], $5::jsonb[])
                 on conflict (id) do nothing",
            )
            .bind(&ids)
            .bind(&hashes)
            .bind(&systems)
            .bind(&times)
            .bind(&orgs_json)
            .execute(pool)
            .await?
            .rows_affected();
            inserted += n as usize;
        }
        println!("{} killmails ({} new)", files.len(), inserted);

        std::fs::remove_file(&archive).ok();
        std::fs::remove_dir_all(&extract_dir).ok();
    }

    println!("running threat analysis…");
    analyze(pool, esi).await?;
    println!("backfill complete.");
    Ok(())
}

type BoxError = Box<dyn std::error::Error>;

fn collect_json_files(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_json_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_match_legacy() {
        assert_eq!(threat_level(0), "unknown");
        assert_eq!(threat_level(14), "unknown");
        assert_eq!(threat_level(15), "high");
        assert_eq!(threat_level(49), "high");
        assert_eq!(threat_level(50), "critical");
    }

    #[test]
    fn orgs_prefer_alliance_and_dedupe() {
        let esi = serde_json::json!({
            "victim": {"corporation_id": 100, "alliance_id": 200},
            "attackers": [
                {"corporation_id": 100, "alliance_id": 200},
                {"corporation_id": 300},
                {"corporation_id": 300},
                {"ship_type_id": 1}
            ]
        });
        let orgs = extract_orgs(&esi);
        assert_eq!(
            orgs,
            vec![
                Org {
                    id: 200,
                    kind: "alliance".into()
                },
                Org {
                    id: 300,
                    kind: "corporation".into()
                },
            ]
        );
    }
}

#[cfg(test)]
mod detail_tests {
    use super::*;

    fn payload() -> (serde_json::Value, serde_json::Value) {
        let esi = serde_json::json!({
            "victim": {
                "character_id": 11, "corporation_id": 22, "alliance_id": 33,
                "ship_type_id": 670, "damage_taken": 1234
            },
            "attackers": [
                {"character_id": 44, "corporation_id": 55, "final_blow": false,
                 "ship_type_id": 111},
                {"character_id": 66, "corporation_id": 77, "alliance_id": 88,
                 "final_blow": true, "ship_type_id": 222}
            ]
        });
        let zkb = serde_json::json!({
            "totalValue": 1_234_567.89, "attackerCount": 2, "npc": false, "solo": false
        });
        (esi, zkb)
    }

    #[test]
    fn a_killmail_yields_what_a_reader_wants() {
        let (esi, zkb) = payload();
        let d = extract_detail(&esi, &zkb);
        assert_eq!(d.victim_character_id, Some(11));
        assert_eq!(d.victim_ship_type_id, Some(670));
        assert_eq!(d.total_value, Some(1_234_567.89));
        assert_eq!(d.attacker_count, Some(2));
        // The killing blow, not merely the first or last attacker.
        assert_eq!(d.final_blow_character_id, Some(66));
        assert_eq!(d.final_blow_alliance_id, Some(88));
        assert_eq!(d.final_blow_ship_type_id, Some(222));
    }

    #[test]
    fn a_frame_without_zkb_still_gives_an_attacker_count() {
        let (esi, _) = payload();
        let d = extract_detail(&esi, &serde_json::Value::Null);
        // Falls back to counting the array, so the row is never mysteriously blank.
        assert_eq!(d.attacker_count, Some(2));
        assert_eq!(d.total_value, None);
        assert!(!d.is_npc && !d.is_solo);
    }

    #[test]
    fn a_kill_with_nobody_flagged_names_no_attacker() {
        let esi = serde_json::json!({ "victim": {}, "attackers": [{"character_id": 1}] });
        let d = extract_detail(&esi, &serde_json::Value::Null);
        assert_eq!(d.final_blow_character_id, None);
        assert_eq!(d.attacker_count, Some(1));
    }
}
