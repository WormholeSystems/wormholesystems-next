//! The map command journal: reading the history and undoing entries.
//!
//! Rows are written by [`command::execute`](super::command::execute) inside the same
//! transaction as the change they describe, so the log can never drift from the state.
//! An entry's `inverse` is a serialized [`MapCommand`], which makes undo a normal
//! command execution and redo an undo of the undo row.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::access::require_role;
use super::command::{Effect, EventActor, MapCommand, execute_as};
use super::error::{MapError, Result};
use super::{Actor, Role};

/// How long an entry stays undoable (and stored).
pub const RETENTION_DAYS: i32 = 7;
/// How many entries the history endpoint returns.
const HISTORY_LIMIT: i64 = 25;

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapEventEntry {
    pub id: i64,
    pub map_id: i64,
    pub character_id: Option<i64>,
    /// Actor name, or `None` for background tasks.
    pub character_name: Option<String>,
    pub kind: String,
    pub label: String,
    pub entries_count: i32,
    /// Whether this entry carries an inverse and has not been undone yet.
    pub undoable: bool,
    pub undone_at: Option<DateTime<Utc>>,
    /// The entry this one reverted, if any (undo/redo chains).
    pub reverts_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Write the journal row for an applied command. Called only from
/// [`command::execute_as`](super::command::execute_as), inside its transaction.
pub(super) async fn record(
    tx: &mut super::command::Tx<'_>,
    map_id: i64,
    actor: EventActor,
    effect: &Effect,
    reverts_id: Option<i64>,
) -> Result<()> {
    let inverse = match &effect.inverse {
        Some(cmd) => Some(serde_json::to_value(cmd).map_err(|e| {
            MapError::Validation(format!("could not serialize the inverse command: {e}"))
        })?),
        None => None,
    };
    sqlx::query!(
        "insert into map_events
             (map_id, character_id, kind, label, entries_count, inverse, reverts_id)
         values ($1, $2, $3, $4, $5, $6, $7)",
        map_id,
        actor.character_id(),
        effect.kind,
        effect.label,
        effect.entries as i32,
        inverse,
        reverts_id,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The map's recent history, newest first. Viewer+.
pub async fn list_events(pool: &PgPool, actor: Actor, map_id: i64) -> Result<Vec<MapEventEntry>> {
    require_role(pool, map_id, actor.user_id, Role::Viewer).await?;
    let entries = sqlx::query_as!(
        MapEventEntry,
        r#"select e.id, e.map_id, e.character_id, c.name as "character_name?",
                  e.kind, e.label, e.entries_count,
                  (e.inverse is not null
                   and e.undone_at is null
                   and e.created_at > now() - make_interval(days => $3)) as "undoable!",
                  e.undone_at, e.reverts_id, e.created_at
           from map_events e
           left join characters c on c.id = e.character_id
           where e.map_id = $1
           order by e.id desc
           limit $2"#,
        map_id,
        HISTORY_LIMIT,
        RETENTION_DAYS,
    )
    .fetch_all(pool)
    .await?;
    Ok(entries)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UndoMapEvent {
    pub map_id: i64,
    pub event_id: i64,
}

/// Revert one entry by executing its inverse. Member+, and only entries the acting
/// character created themselves (legacy rule: you cannot undo someone else's work).
/// The reverting execution records its own row pointing back at this one, so undoing
/// *that* row is the redo.
pub async fn undo(pool: &PgPool, actor: Actor, cmd: UndoMapEvent) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let row = sqlx::query!(
        r#"select character_id, inverse, undone_at,
                  created_at < now() - make_interval(days => $3) as "expired!"
           from map_events where id = $1 and map_id = $2"#,
        cmd.event_id,
        cmd.map_id,
        RETENTION_DAYS,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)?;

    if row.character_id != Some(actor.character_id) {
        return Err(MapError::Forbidden);
    }
    if row.undone_at.is_some() {
        return Err(MapError::Conflict("that change was already undone".into()));
    }
    if row.expired {
        return Err(MapError::Conflict(
            "that change is older than the undo window".into(),
        ));
    }
    let Some(inverse) = row.inverse else {
        return Err(MapError::Conflict("that change cannot be undone".into()));
    };
    let inverse: MapCommand = serde_json::from_value(inverse)
        .map_err(|e| MapError::Validation(format!("corrupt undo entry: {e}")))?;

    execute_as(
        pool,
        EventActor::Character(actor),
        inverse,
        Some(cmd.event_id),
    )
    .await?;

    sqlx::query!(
        "update map_events set undone_at = now() where id = $1",
        cmd.event_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Drop entries past the retention window.
pub async fn purge(pool: &PgPool) -> Result<u64> {
    let deleted = sqlx::query!(
        "delete from map_events where created_at < now() - make_interval(days => $1)",
        RETENTION_DAYS,
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(deleted)
}

/// Spawn the daily history purge.
pub fn start_purge(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(6 * 60 * 60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(err) = purge(&pool).await {
                eprintln!("map event purge failed: {err}");
            }
        }
    });
}
