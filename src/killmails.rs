//! Killmail ingest and threat analysis.
//!
//! Ingest polls zKillboard's R2Z2 stream for a minimal row per killmail; the full ESI
//! payload is not kept. Backfill imports EVE Ref's daily archives for the same 90-day
//! window the analysis looks over, since the live stream starts blind.
//!
//! Analysis, daily, per wormhole system over 90 days: count kills per organisation (victim
//! and attackers, alliance over corporation, each org once per killmail), keep those active
//! on >= 5 distinct days, take the top 10. Summed kills set the level: >= 50 critical,
//! >= 15 high, else unknown.
//!
//! All three loops only run when `ZKB_LISTEN=1`.

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
/// How many rows the card asks for. A recent feed, not an archive.
pub const CARD_LIMIT: i64 = 60;
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
    esi: EsiKillmail,
    /// zKillboard's own summary: value, attacker count, and the solo/NPC flags. Absent on
    /// a malformed frame, which is why every field below is read defensively.
    #[serde(default)]
    zkb: serde_json::Value,
}

/// A killmail's ESI body, narrowed to the fields anything here reads.
///
/// Typed rather than `serde_json::Value`: the full body lists every item the victim carried,
/// and against a year of archives those allocations dominate the import. Every field is
/// optional so a malformed payload costs one killmail rather than a batch.
#[derive(Default, Deserialize)]
struct EsiKillmail {
    killmail_id: Option<i64>,
    killmail_hash: Option<String>,
    killmail_time: Option<String>,
    solar_system_id: Option<i64>,
    #[serde(default)]
    victim: Participant,
    #[serde(default)]
    attackers: Vec<Participant>,
}

/// One side of a killmail; the victim never carries `final_blow`, so both share a shape.
#[derive(Default, Deserialize)]
struct Participant {
    character_id: Option<i64>,
    corporation_id: Option<i64>,
    alliance_id: Option<i64>,
    ship_type_id: Option<i64>,
    #[serde(default)]
    final_blow: bool,
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

fn extract_detail(esi: &EsiKillmail, zkb: &serde_json::Value) -> Detail {
    let victim = &esi.victim;
    // The killing blow is the one attacker worth naming; the rest are a count.
    let final_blow = esi.attackers.iter().find(|a| a.final_blow);
    Detail {
        victim_character_id: victim.character_id,
        victim_corporation_id: victim.corporation_id,
        victim_alliance_id: victim.alliance_id,
        victim_ship_type_id: victim.ship_type_id,
        total_value: zkb["totalValue"].as_f64(),
        attacker_count: Some(
            zkb["attackerCount"]
                .as_i64()
                .unwrap_or(esi.attackers.len() as i64) as i32,
        ),
        is_npc: zkb["npc"].as_bool().unwrap_or(false),
        is_solo: zkb["solo"].as_bool().unwrap_or(false),
        final_blow_character_id: final_blow.and_then(|a| a.character_id),
        final_blow_corporation_id: final_blow.and_then(|a| a.corporation_id),
        final_blow_alliance_id: final_blow.and_then(|a| a.alliance_id),
        final_blow_ship_type_id: final_blow.and_then(|a| a.ship_type_id),
    }
}

/// zKillboard and EVE Ref reject anonymous clients (403), so identify ourselves.
fn http_client() -> reqwest::Client {
    crate::user_agent::client()
}

/// Spawn the ingest, backfill and analysis loops (gated by `ZKB_LISTEN=1`).
/// `KILLMAIL_BACKFILL_DAYS` overrides how much history a boot fills in; 0 turns it off.
pub fn start(
    pool: PgPool,
    esi: EsiClient,
    maps: crate::maps::MapHub,
    alerts: Option<std::sync::Arc<crate::alerts::Runtime>>,
) {
    if std::env::var("ZKB_LISTEN").as_deref() != Ok("1") {
        return;
    }
    let base = std::env::var("ZKB_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let days = std::env::var("KILLMAIL_BACKFILL_DAYS")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(BACKFILL_DAYS);
    tokio::spawn(listen(pool.clone(), base, maps, alerts));
    tokio::spawn(resolve_loop(pool.clone(), esi.clone()));
    if days > 0 {
        tokio::spawn(backfill_loop(pool.clone(), esi.clone(), days));
    }
    tokio::spawn(analysis_loop(pool, esi));
}

async fn listen(
    pool: PgPool,
    base: String,
    maps: crate::maps::MapHub,
    alerts: Option<std::sync::Arc<crate::alerts::Runtime>>,
) {
    let http = http_client();
    loop {
        match ingest_next(&pool, &http, &base, &maps, alerts.as_deref()).await {
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
    /// Spelled out for the tooltip; the row has room for a ticker at most.
    #[ts(optional)]
    pub corporation_name: Option<String>,
    #[ts(optional)]
    pub alliance_id: Option<i64>,
    #[ts(optional)]
    pub alliance_ticker: Option<String>,
    #[ts(optional)]
    pub alliance_name: Option<String>,
    #[ts(optional)]
    pub ship_type_id: Option<i64>,
    #[ts(optional)]
    pub ship_name: Option<String>,
}

/// A killmail as the card shows it: what a row renders, not the raw payload.
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

/// Recent kills in the systems currently on a map, newest first. Bounded by time as well
/// as count: a row cap alone shows a quiet chain kills from a year ago as though new.
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
                  vco.name as "victim_corporation_name?",
                  k.victim_alliance_id, va.ticker as "victim_alliance_ticker?",
                  va.name as "victim_alliance_name?",
                  k.victim_ship_type_id, vt.name as "victim_ship_name?",
                  k.final_blow_character_id, fc.name as "final_blow_character_name?",
                  k.final_blow_corporation_id, fco.ticker as "final_blow_corporation_ticker?",
                  fco.name as "final_blow_corporation_name?",
                  k.final_blow_alliance_id, fa.ticker as "final_blow_alliance_ticker?",
                  fa.name as "final_blow_alliance_name?",
                  k.final_blow_ship_type_id, ft.name as "final_blow_ship_name?"
           from killmails k
           join map_solar_systems mss
               on mss.map_id = $1 and mss.solar_system_id = k.solar_system_id
           join solar_systems ss on ss.id = k.solar_system_id
           join constellations c on c.id = ss.constellation_id
           join regions r on r.id = c.region_id
           left join wormhole_systems ws on ws.solar_system_id = ss.id
           left join characters vc on vc.id = k.victim_character_id
           left join corporations vco on vco.id = k.victim_corporation_id
           left join alliances va on va.id = k.victim_alliance_id
           left join types vt on vt.id = k.victim_ship_type_id
           left join characters fc on fc.id = k.final_blow_character_id
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
                corporation_name: r.victim_corporation_name,
                alliance_id: r.victim_alliance_id,
                alliance_ticker: r.victim_alliance_ticker,
                alliance_name: r.victim_alliance_name,
                ship_type_id: r.victim_ship_type_id,
                ship_name: r.victim_ship_name,
            },
            final_blow: KillParty {
                character_id: r.final_blow_character_id,
                character_name: r.final_blow_character_name,
                corporation_id: r.final_blow_corporation_id,
                corporation_ticker: r.final_blow_corporation_ticker,
                corporation_name: r.final_blow_corporation_name,
                alliance_id: r.final_blow_alliance_id,
                alliance_ticker: r.final_blow_alliance_ticker,
                alliance_name: r.final_blow_alliance_name,
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

/// Put names to the ids on recent killmails. A separate loop from the ingest: a killmail
/// must be recorded whether or not ESI is answering, and the names are only wanted by the
/// time someone opens the card.
async fn resolve_loop(pool: PgPool, esi: EsiClient) {
    loop {
        resolve_recent(&pool, &esi).await;
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn resolve_recent(pool: &PgPool, esi: &EsiClient) {
    // Windowed to what the card can show; `distinct` because a busy system repeats orgs.
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
    // Organisations first: most rows named per fetch if a rate limit cuts the run short.
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
    alerts: Option<&crate::alerts::Runtime>,
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

    let solar_system_id = km.esi.solar_system_id.unwrap_or(0);
    let time = km.esi.killmail_time.clone().unwrap_or_default();
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
        if let Some(alerts) = alerts {
            alert_on(pool, alerts, km.killmail_id, solar_system_id, &detail).await;
        }
    }
    advance(pool, next).await?;
    Ok(true)
}

/// Offer the kill to the Discord alerts watching for one. Names come from what is already
/// stored, since "Someone lost a Loki" now beats a complete message a minute later.
async fn alert_on(
    pool: &PgPool,
    alerts: &crate::alerts::Runtime,
    killmail_id: i64,
    solar_system_id: i64,
    detail: &Detail,
) {
    use crate::alerts::filters::Candidates;
    let named = sqlx::query!(
        r#"select vc.name as "victim_name?", vt.name as "victim_ship?",
                  fc.name as "attacker_name?", vt.group_id as "victim_ship_group?",
                  ft.group_id as "attacker_ship_group?"
           from (select 1) as one
           left join characters vc on vc.id = $1
           left join characters fc on fc.id = $2
           left join types vt on vt.id = $3
           left join types ft on ft.id = $4"#,
        detail.victim_character_id,
        detail.final_blow_character_id,
        detail.victim_ship_type_id,
        detail.final_blow_ship_type_id,
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let kill = crate::alerts::killmail::Kill {
        id: killmail_id,
        solar_system_id,
        candidates: Candidates {
            victim_character: detail.victim_character_id,
            victim_corporation: detail.victim_corporation_id,
            victim_alliance: detail.victim_alliance_id,
            victim_ship_type: detail.victim_ship_type_id,
            victim_ship_group: named.as_ref().and_then(|n| n.victim_ship_group),
            attacker_character: detail.final_blow_character_id,
            attacker_corporation: detail.final_blow_corporation_id,
            attacker_alliance: detail.final_blow_alliance_id,
            attacker_ship_type: detail.final_blow_ship_type_id,
            attacker_ship_group: named.as_ref().and_then(|n| n.attacker_ship_group),
        },
        victim_name: named.as_ref().and_then(|n| n.victim_name.clone()),
        victim_ship: named.as_ref().and_then(|n| n.victim_ship.clone()),
        victim_ship_type_id: detail.victim_ship_type_id,
        attacker_name: named.as_ref().and_then(|n| n.attacker_name.clone()),
        total_value: detail.total_value,
        attacker_count: detail.attacker_count,
        is_solo: detail.is_solo,
        is_npc: detail.is_npc,
    };
    alerts.killmail(pool, &kill).await;
}

/// Tell every map holding this system that something died in it. The event carries no
/// payload: what a client shows depends on its own filter, so it refetches.
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
fn extract_orgs(esi: &EsiKillmail) -> Vec<Org> {
    let mut seen = std::collections::HashSet::new();
    let mut orgs = Vec::new();
    let mut push = |entity: &Participant| {
        let (id, alliance) = match entity.alliance_id {
            Some(id) => (id, true),
            None => match entity.corporation_id {
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
    push(&esi.victim);
    for attacker in &esi.attackers {
        push(attacker);
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

/// One killmail from an archive, reduced to exactly what the insert binds.
struct ArchivedKill {
    id: i64,
    hash: String,
    solar_system_id: i64,
    time: String,
    orgs: serde_json::Value,
    detail: Detail,
}

/// Turn a downloaded `.tar.bz2` into rows, without touching the disk. A day is around 24,000
/// separate JSON files, and writing them out to extract them cost more than the decompression
/// and the insert put together.
///
/// Blocking and CPU-bound, so callers run it off the async runtime.
fn read_archive(bytes: &[u8]) -> std::io::Result<Vec<ArchivedKill>> {
    use std::io::Read;

    // Multi-stream: the archives are concatenated bzip2 streams, and a plain decoder would
    // stop at the end of the first one and silently return part of the day.
    let mut archive = tar::Archive::new(bzip2::read::MultiBzDecoder::new(bytes));
    let mut kills = Vec::new();
    let mut text = String::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()?.extension().is_none_or(|e| e != "json") {
            continue;
        }
        text.clear();
        if entry.read_to_string(&mut text).is_err() {
            continue;
        }
        let Ok(km) = serde_json::from_str::<EsiKillmail>(&text) else {
            continue;
        };
        let (Some(id), Some(solar_system_id), Some(time)) =
            (km.killmail_id, km.solar_system_id, km.killmail_time.clone())
        else {
            continue;
        };
        kills.push(ArchivedKill {
            id,
            hash: km.killmail_hash.clone().unwrap_or_default(),
            solar_system_id,
            time,
            orgs: serde_json::to_value(extract_orgs(&km)).map_err(std::io::Error::other)?,
            // The archives carry the ESI body but not zKillboard's block, so everything but
            // the ISK value and the solo/NPC flags comes through. The card ignores rows with
            // no victim ship, so storing only the bare minimum would import invisible history.
            detail: extract_detail(&km, &serde_json::Value::Null),
        });
    }
    Ok(kills)
}

/// How far back a boot fills, matching the window threat analysis looks over. The live
/// listener only ever sees kills from now on, so without this a fresh instance is useless
/// for months.
pub const BACKFILL_DAYS: u32 = 90;

/// The days in the last `days` that are not imported yet, most recent first.
async fn missing_days(pool: &PgPool, days: u32) -> Result<Vec<chrono::NaiveDate>, BoxError> {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let wanted: Vec<chrono::NaiveDate> = (1..=i64::from(days))
        .map(|offset| {
            chrono::DateTime::from_timestamp(now_secs - offset * 86_400, 0)
                .expect("valid timestamp")
                .date_naive()
        })
        .collect();
    let done: std::collections::HashSet<chrono::NaiveDate> = sqlx::query_scalar!(
        "select day from killmail_imports where day = any($1)",
        &wanted
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();
    Ok(wanted.into_iter().filter(|d| !done.contains(d)).collect())
}

/// Import one archived day. `Ok(None)` when EVE Ref has nothing for it yet.
async fn import_day(
    pool: &PgPool,
    esi: &EsiClient,
    http: &reqwest::Client,
    day: chrono::NaiveDate,
) -> Result<Option<String>, BoxError> {
    let name = format!("killmails-{}.tar.bz2", day.format("%Y-%m-%d"));
    let url = format!(
        "https://data.everef.net/killmails/{}/{name}",
        day.format("%Y")
    );

    let res = http.get(&url).send().await?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let bytes = res.error_for_status()?.bytes().await?;
    let kills = tokio::task::spawn_blocking(move || read_archive(&bytes)).await??;

    // Every entity the day mentions, deduped before anything is fetched: tens of thousands
    // of killmails name only a few thousand distinct pilots.
    let mut day_characters: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut day_corporations: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut day_alliances: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for k in &kills {
        day_characters.extend(k.detail.victim_character_id);
        day_characters.extend(k.detail.final_blow_character_id);
        day_corporations.extend(k.detail.victim_corporation_id);
        day_corporations.extend(k.detail.final_blow_corporation_id);
        day_alliances.extend(k.detail.victim_alliance_id);
        day_alliances.extend(k.detail.final_blow_alliance_id);
    }

    let mut inserted = 0usize;
    // The statement binds fixed arrays rather than one placeholder per value, so the batch
    // is bounded by memory rather than by Postgres's parameter limit.
    for chunk in kills.chunks(2_000) {
        let mut ids = Vec::new();
        let mut hashes = Vec::new();
        let mut systems = Vec::new();
        let mut times = Vec::new();
        let mut orgs_json = Vec::new();
        let mut victim_chars: Vec<Option<i64>> = Vec::new();
        let mut victim_corps: Vec<Option<i64>> = Vec::new();
        let mut victim_allis: Vec<Option<i64>> = Vec::new();
        let mut victim_ships: Vec<Option<i64>> = Vec::new();
        let mut values: Vec<Option<f64>> = Vec::new();
        let mut attackers: Vec<Option<i32>> = Vec::new();
        let mut npcs: Vec<bool> = Vec::new();
        let mut solos: Vec<bool> = Vec::new();
        let mut fb_chars: Vec<Option<i64>> = Vec::new();
        let mut fb_corps: Vec<Option<i64>> = Vec::new();
        let mut fb_allis: Vec<Option<i64>> = Vec::new();
        let mut fb_ships: Vec<Option<i64>> = Vec::new();
        for km in chunk {
            let d = &km.detail;
            ids.push(km.id);
            hashes.push(km.hash.clone());
            systems.push(km.solar_system_id);
            times.push(km.time.clone());
            orgs_json.push(km.orgs.clone());

            victim_chars.push(d.victim_character_id);
            victim_corps.push(d.victim_corporation_id);
            victim_allis.push(d.victim_alliance_id);
            victim_ships.push(d.victim_ship_type_id);
            values.push(d.total_value);
            attackers.push(d.attacker_count);
            npcs.push(d.is_npc);
            solos.push(d.is_solo);
            fb_chars.push(d.final_blow_character_id);
            fb_corps.push(d.final_blow_corporation_id);
            fb_allis.push(d.final_blow_alliance_id);
            fb_ships.push(d.final_blow_ship_type_id);
        }
        let n = sqlx::query(
            "insert into killmails (
                 id, hash, solar_system_id, time, orgs,
                 victim_character_id, victim_corporation_id, victim_alliance_id,
                 victim_ship_type_id, total_value, attacker_count, is_npc, is_solo,
                 final_blow_character_id, final_blow_corporation_id,
                 final_blow_alliance_id, final_blow_ship_type_id
             )
             select * from unnest($1::bigint[], $2::text[], $3::bigint[],
                                  $4::text[]::timestamptz[], $5::jsonb[],
                                  $6::bigint[], $7::bigint[], $8::bigint[], $9::bigint[],
                                  $10::double precision[], $11::int[], $12::boolean[],
                                  $13::boolean[], $14::bigint[], $15::bigint[],
                                  $16::bigint[], $17::bigint[])
             on conflict (id) do nothing",
        )
        .bind(&ids)
        .bind(&hashes)
        .bind(&systems)
        .bind(&times)
        .bind(&orgs_json)
        .bind(&victim_chars)
        .bind(&victim_corps)
        .bind(&victim_allis)
        .bind(&victim_ships)
        .bind(&values)
        .bind(&attackers)
        .bind(&npcs)
        .bind(&solos)
        .bind(&fb_chars)
        .bind(&fb_corps)
        .bind(&fb_allis)
        .bind(&fb_ships)
        .execute(pool)
        .await?
        .rows_affected();
        inserted += n as usize;
    }
    // Organisations one at a time, because their tickers are what a row shows and only the
    // per-entity endpoint returns them. Characters go through the bulk endpoint.
    let alliances: Vec<i64> = day_alliances.into_iter().collect();
    let corporations: Vec<i64> = day_corporations.into_iter().collect();
    let characters: Vec<i64> = day_characters.into_iter().collect();
    crate::entities::ensure(pool, esi, EntityKind::Alliance, &alliances).await;
    crate::entities::ensure(pool, esi, EntityKind::Corporation, &corporations).await;
    let named = crate::entities::ensure_character_names(pool, esi, &characters).await;

    sqlx::query!(
        "insert into killmail_imports (day, killmails) values ($1, $2)
         on conflict (day) do update set killmails = excluded.killmails, imported_at = now()",
        day,
        kills.len() as i32,
    )
    .execute(pool)
    .await?;

    Ok(Some(format!(
        "{} killmails ({inserted} new), named {named} of {} pilots and resolved {} orgs",
        kills.len(),
        characters.len(),
        alliances.len() + corporations.len()
    )))
}

/// Backfill killmails from EVE Ref's daily archives (`wormholesystems killmails-backfill <days>`),
/// most recent day first. Days already in the ledger are skipped, so re-running only fetches
/// what is missing. Ends with a threat analysis run so the data shows up immediately.
pub async fn backfill(pool: &PgPool, esi: &EsiClient, days: u32) -> Result<(), BoxError> {
    let http = http_client();
    let missing = missing_days(pool, days).await?;
    println!("{} of the last {days} days to import", missing.len());
    for day in missing {
        print!("{day}: downloading… ");
        use std::io::Write;
        std::io::stdout().flush().ok();
        match import_day(pool, esi, &http, day).await? {
            Some(summary) => println!("{summary}"),
            None => println!("no archive"),
        }
    }

    println!("running threat analysis…");
    analyze(pool, esi).await?;
    println!("backfill complete.");
    Ok(())
}

/// Background: fill in the archived days this instance is missing. Once per boot rather than
/// on a schedule, because the live listener covers everything from startup onwards.
async fn backfill_loop(pool: PgPool, esi: EsiClient, days: u32) {
    let http = http_client();
    let missing = match missing_days(&pool, days).await {
        Ok(days) => days,
        Err(err) => return eprintln!("killmail backfill: {err}"),
    };
    if missing.is_empty() {
        return;
    }
    println!("killmail backfill: {} days missing", missing.len());
    let mut imported = 0usize;
    for day in missing {
        match import_day(&pool, &esi, &http, day).await {
            Ok(Some(_)) => imported += 1,
            Ok(None) => {}
            Err(err) => eprintln!("killmail backfill {day}: {err}"),
        }
    }
    println!("killmail backfill: {imported} days imported");
    if imported > 0
        && let Err(err) = analyze(&pool, &esi).await
    {
        eprintln!("threat analysis after backfill failed: {err}");
    }
}

type BoxError = Box<dyn std::error::Error>;

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

    /// Tests state the payload as it arrives on the wire, then read it the way the code
    /// does, so a field the struct forgets shows up here.
    fn body(value: serde_json::Value) -> EsiKillmail {
        serde_json::from_value(value).expect("killmail body")
    }

    #[test]
    fn orgs_prefer_alliance_and_dedupe() {
        let esi = body(serde_json::json!({
            "victim": {"corporation_id": 100, "alliance_id": 200},
            "attackers": [
                {"corporation_id": 100, "alliance_id": 200},
                {"corporation_id": 300},
                {"corporation_id": 300},
                {"ship_type_id": 1}
            ]
        }));
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

    fn body(value: serde_json::Value) -> EsiKillmail {
        serde_json::from_value(value).expect("killmail body")
    }

    fn payload() -> (EsiKillmail, serde_json::Value) {
        let esi = body(serde_json::json!({
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
        }));
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
        let esi = body(serde_json::json!({ "victim": {}, "attackers": [{"character_id": 1}] }));
        let d = extract_detail(&esi, &serde_json::Value::Null);
        assert_eq!(d.final_blow_character_id, None);
        assert_eq!(d.attacker_count, Some(1));
    }
}
