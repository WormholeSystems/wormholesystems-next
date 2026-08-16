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

use crate::esi::EsiClient;

const DEFAULT_BASE: &str = "https://r2z2.zkillboard.com";
const RETENTION_DAYS: i32 = 730;
const ANALYSIS_WINDOW_DAYS: i32 = 90;
const MIN_ACTIVE_DAYS: i64 = 5;
const TOP_ORGS: i64 = 10;
const HOSTILE_THRESHOLD: i64 = 50;
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
}

/// Spawn the ingest + analysis loops (gated by `ZKB_LISTEN=1`).
pub fn start(pool: PgPool, esi: EsiClient) {
    if std::env::var("ZKB_LISTEN").as_deref() != Ok("1") {
        return;
    }
    let base = std::env::var("ZKB_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    tokio::spawn(listen(pool.clone(), base));
    tokio::spawn(analysis_loop(pool, esi));
}

async fn listen(pool: PgPool, base: String) {
    let http = reqwest::Client::new();
    loop {
        match ingest_next(&pool, &http, &base).await {
            Ok(true) => tokio::time::sleep(Duration::from_millis(500)).await,
            Ok(false) => tokio::time::sleep(Duration::from_secs(10)).await,
            Err(err) => {
                eprintln!("killmail ingest error: {err}");
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }
}

/// Fetch and persist the next killmail in the sequence. Returns whether one was found.
async fn ingest_next(
    pool: &PgPool,
    http: &reqwest::Client,
    base: &str,
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
    if solar_system_id != 0 && !time.is_empty() {
        // The retention check happens in SQL (chrono's clock is disabled in this crate).
        sqlx::query(
            "insert into killmails (id, hash, solar_system_id, time, orgs)
             select $1, $2, $3, $4::timestamptz, $5
             where $4::timestamptz >= now() - make_interval(days => $6)
             on conflict (id) do nothing",
        )
        .bind(km.killmail_id)
        .bind(&km.hash)
        .bind(solar_system_id)
        .bind(time)
        .bind(serde_json::to_value(&orgs)?)
        .bind(RETENTION_DAYS)
        .execute(pool)
        .await?;
    }
    advance(pool, next).await?;
    Ok(true)
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
