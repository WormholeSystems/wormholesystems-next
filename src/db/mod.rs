use std::time::SystemTime;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::esi::token::{Token, TokenStore};
use crate::esi::{EsiError, Result as EsiResult};

/// How many Postgres connections to hold open. The default is not sqlx's five: the API,
/// every open websocket, the tracking poller, killmail ingest and the alert lifecycle all
/// draw on one pool, and the poller alone asks for more than five at a time — so five means
/// requests queue behind background work rather than being served.
const DEFAULT_MAX_CONNECTIONS: u32 = 20;

/// Connect to Postgres and run any pending migrations.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let max = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_CONNECTIONS);
    let pool = PgPoolOptions::new()
        .max_connections(max)
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Postgres-backed [`TokenStore`].
#[derive(Clone)]
pub struct PgTokenStore {
    pool: PgPool,
}

impl PgTokenStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// Lets the `TokenStore` impl use `?` on sqlx errors directly.
impl From<sqlx::Error> for EsiError {
    fn from(e: sqlx::Error) -> Self {
        EsiError::Store(e.to_string())
    }
}

impl TokenStore for PgTokenStore {
    async fn load(&self, character_id: i64) -> EsiResult<Option<Token>> {
        let row = sqlx::query!(
            "SELECT id, access_token, token_expires_at, refresh_token
             FROM esi_tokens WHERE character_id = $1
             ORDER BY updated_at DESC LIMIT 1",
            character_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };

        let scopes = sqlx::query_scalar!(
            "SELECT s.name FROM esi_token_scopes ts
             JOIN esi_scopes s ON s.id = ts.scope_id
             WHERE ts.token_id = $1",
            row.id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(Token {
            access_token: row.access_token.unwrap_or_default(),
            refresh_token: row.refresh_token,
            // A null/absent expiry is treated as already expired (forces a refresh).
            expires_at: row
                .token_expires_at
                .map(SystemTime::from)
                .unwrap_or(SystemTime::UNIX_EPOCH),
            scopes,
        }))
    }

    async fn save(&self, character_id: i64, token: &Token) -> EsiResult<()> {
        let expires_at = DateTime::<Utc>::from(token.expires_at);

        let mut tx = self.pool.begin().await?;

        // The store keeps one token per character; replace any existing one.
        sqlx::query!(
            "DELETE FROM esi_tokens WHERE character_id = $1",
            character_id
        )
        .execute(&mut *tx)
        .await?;

        let token_id = sqlx::query_scalar!(
            "INSERT INTO esi_tokens (character_id, access_token, token_expires_at, refresh_token)
             VALUES ($1, $2, $3, $4) RETURNING id",
            character_id,
            token.access_token,
            expires_at,
            token.refresh_token
        )
        .fetch_one(&mut *tx)
        .await?;

        for scope in &token.scopes {
            let scope_id = sqlx::query_scalar!(
                "INSERT INTO esi_scopes (name) VALUES ($1)
                 ON CONFLICT (name) DO UPDATE SET name = excluded.name
                 RETURNING id",
                scope
            )
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query!(
                "INSERT INTO esi_token_scopes (token_id, scope_id) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
                token_id,
                scope_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
