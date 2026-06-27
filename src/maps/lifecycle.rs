//! Map lifecycle actions: create, update, delete, list, and read the graph.

use sqlx::PgPool;

use super::access::{owns_character, require_role};
use super::error::{MapError, Result};
use super::{
    Actor, ConnectionType, Map, MapConnection, MapSolarSystem, MapView, Role, SubjectType,
};

/// Create a map and grant ownership to the acting character. Any authenticated user may
/// create a map.
pub async fn create_map(
    pool: &PgPool,
    actor: Actor,
    name: &str,
    description: Option<&str>,
) -> Result<Map> {
    let name = name.trim();
    if name.is_empty() {
        return Err(MapError::Validation("name must not be blank".into()));
    }
    if !owns_character(pool, actor.user_id, actor.character_id).await? {
        return Err(MapError::Forbidden);
    }

    let mut tx = pool.begin().await?;
    let map = sqlx::query_as!(
        Map,
        "insert into maps (name, description) values ($1, $2)
         returning id, name, description, image_url, created_at",
        name,
        description,
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "insert into map_access (map_id, subject_type, subject_id, role)
         values ($1, $2, $3, $4)",
        map.id,
        SubjectType::Character.as_str(),
        actor.character_id,
        Role::Owner.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(map)
}

/// A partial update of a map's fields. `None` leaves a field unchanged; `Some(None)`
/// explicitly clears a nullable field.
#[derive(Debug, Default, Clone)]
pub struct MapUpdate {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub image_url: Option<Option<String>>,
}

/// Rename / re-describe / re-icon a map. Owner only.
pub async fn update_map(pool: &PgPool, actor: Actor, map_id: i64, patch: MapUpdate) -> Result<Map> {
    require_role(pool, map_id, actor.user_id, Role::Owner).await?;

    let mut tx = pool.begin().await?;
    let current = sqlx::query_as!(
        Map,
        "select id, name, description, image_url, created_at from maps where id = $1",
        map_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(MapError::NotFound)?;

    let name = match patch.name {
        Some(name) => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(MapError::Validation("name must not be blank".into()));
            }
            name
        }
        None => current.name,
    };
    let description = patch.description.unwrap_or(current.description);
    let image_url = patch.image_url.unwrap_or(current.image_url);

    let map = sqlx::query_as!(
        Map,
        "update maps set name = $1, description = $2, image_url = $3 where id = $4
         returning id, name, description, image_url, created_at",
        name,
        description,
        image_url,
        map_id,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(map)
}

/// Delete a map. Owner only. The database cascades placements, details, connections,
/// signatures, and access grants.
pub async fn delete_map(pool: &PgPool, actor: Actor, map_id: i64) -> Result<()> {
    require_role(pool, map_id, actor.user_id, Role::Owner).await?;
    sqlx::query!("delete from maps where id = $1", map_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Every map the user can access, paired with their effective role on it.
pub async fn list_maps(pool: &PgPool, user_id: i64) -> Result<Vec<(Map, Role)>> {
    let rows = sqlx::query!(
        r#"select m.id, m.name, m.description, m.image_url, m.created_at, ma.role as "role!: Role"
           from maps m
           join map_access ma on ma.map_id = m.id
           where ma.subject_id in (
                 select id from characters where user_id = $1
                 union all
                 select corporation_id from characters where user_id = $1
                 union all
                 select alliance_id from characters where user_id = $1 and alliance_id is not null
           )
           order by m.created_at"#,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    // A map can match more than once (e.g. via a character and its corp); keep the
    // highest role per map.
    let mut out: Vec<(Map, Role)> = Vec::new();
    for r in rows {
        let map = Map {
            id: r.id,
            name: r.name,
            description: r.description,
            image_url: r.image_url,
            created_at: r.created_at,
        };
        match out.iter_mut().find(|(m, _)| m.id == map.id) {
            Some((_, role)) => *role = (*role).max(r.role),
            None => out.push((map, r.role)),
        }
    }
    Ok(out)
}

/// Read a map's graph — the map, its placed systems, and its connections. Viewer+.
/// Live pilot locations are not included (that's the member-gated tracking path).
pub async fn get_map(pool: &PgPool, actor: Actor, map_id: i64) -> Result<MapView> {
    require_role(pool, map_id, actor.user_id, Role::Viewer).await?;

    let map = sqlx::query_as!(
        Map,
        "select id, name, description, image_url, created_at from maps where id = $1",
        map_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)?;

    let systems = sqlx::query_as!(
        MapSolarSystem,
        "select id, map_id, solar_system_id, position_x, position_y, alias, created_at
         from map_solar_systems where map_id = $1 order by id",
        map_id,
    )
    .fetch_all(pool)
    .await?;

    let connections = sqlx::query_as!(
        MapConnection,
        r#"select id, map_id, from_system, to_system, type as "kind: ConnectionType", created_at
           from map_connections where map_id = $1 order by id"#,
        map_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(MapView {
        map,
        systems,
        connections,
    })
}
