//! Map business logic: the authorized, validated actions a user takes on a map.
//!
//! This is the application layer over the [database spec](../../docs/database/), built
//! to the behaviour spec in [`docs/features/maps.md`](../../docs/features/maps.md).
//! Actions are plain async functions over a `PgPool` — no HTTP/UI here — so they're
//! driven directly from tests and later from server handlers.

use chrono::{DateTime, Utc};

pub mod access;
pub mod error;
pub mod graph;
pub mod lifecycle;

pub use error::{MapError, Result};

/// A Rust enum stored as `text` (per the schema's [enum convention](../../docs/database/README.md)).
/// Generates the variants, `as_str` / `from_db`, and the sqlx glue so the enum binds and
/// decodes directly in queries. Variant order is the `Ord` order — used for `Role`.
macro_rules! text_enum {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        $vis enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn from_db(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$variant),)+ _ => None }
            }
        }

        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <str as sqlx::Type<sqlx::Postgres>>::type_info()
            }
            fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
                <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
            }
        }
        impl<'q> sqlx::Encode<'q, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> std::result::Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <&str as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str(), buf)
            }
        }
        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> std::result::Result<Self, sqlx::error::BoxDynError> {
                let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
                Self::from_db(s).ok_or_else(|| format!("invalid {} value: {s}", stringify!($name)).into())
            }
        }
    };
}

text_enum! {
    /// A map access role, ordered `Viewer < Member < Manager < Owner`.
    pub enum Role {
        Viewer => "viewer",
        Member => "member",
        Manager => "manager",
        Owner => "owner",
    }
}

text_enum! {
    /// What an access grant targets. EVE ids are globally unique across the three.
    pub enum SubjectType {
        Character => "character",
        Corporation => "corporation",
        Alliance => "alliance",
    }
}

text_enum! {
    /// The kind of edge between two placed systems.
    pub enum ConnectionType {
        Wormhole => "wormhole",
        Stargate => "stargate",
    }
}

/// A user acting as one of their characters. `user_id` drives authorization (effective
/// role across all their characters); `character_id` attributes ownership on creation.
#[derive(Debug, Clone, Copy)]
pub struct Actor {
    pub user_id: i64,
    pub character_id: i64,
}

#[derive(Debug, Clone)]
pub struct Map {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MapSolarSystem {
    pub id: i64,
    pub map_id: i64,
    pub solar_system_id: i64,
    pub position_x: f64,
    pub position_y: f64,
    pub alias: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MapConnection {
    pub id: i64,
    pub map_id: i64,
    pub from_system: i64,
    pub to_system: i64,
    pub kind: ConnectionType,
    pub created_at: DateTime<Utc>,
}

/// The graph as seen by a viewer: the map plus its placed systems and connections.
#[derive(Debug, Clone)]
pub struct MapView {
    pub map: Map,
    pub systems: Vec<MapSolarSystem>,
    pub connections: Vec<MapConnection>,
}
