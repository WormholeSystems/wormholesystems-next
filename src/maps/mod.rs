//! Map business logic: the authorized, validated actions a user takes on a map.
//!
//! This is the application layer over the [database spec](../../docs/database/), built
//! to the behaviour spec in [`docs/features/maps.md`](../../docs/features/maps.md).
//! Actions are plain async functions over a `PgPool` — no HTTP/UI here — so they're
//! driven directly from tests and later from server handlers.

// Cross-target modules: each holds shared data types (compiled for ssr + wasm) plus
// `ssr`-gated DB actions. `access` and `error` are server-only.
pub mod access;
pub mod connection;
pub mod error;
pub mod events;
pub mod map;
pub mod signatures;
pub mod solar_system;

pub use connection::MapConnection;
pub use error::{MapError, Result};
pub use events::MapEvent;
pub use events::MapHub;
pub use map::Map;
pub use signatures::Signature;
pub use solar_system::{EffectModifier, MapSolarSystem, MapSystemView, Sovereignty, Static};

/// A Rust enum stored as `text` (per the schema's [enum convention](../../docs/database/README.md)).
/// Generates the variants, `as_str` / `from_db`, and the sqlx glue so the enum binds and
/// decodes directly in queries. Variant order is the `Ord` order — used for `Role`.
macro_rules! text_enum {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize, ts_rs::TS)]
        #[serde(rename_all = "lowercase")]
        #[ts(export)]
        $vis enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn from_db(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$variant),)+ _ => None }
            }
        }

        // Only `Type` + `Decode` are generated: queries bind these enums as `&str` via
        // `as_str()`, but read them back through `as "col: Enum"` casts. Server-only — the
        // sqlx glue isn't compiled for the wasm client.
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <str as sqlx::Type<sqlx::Postgres>>::type_info()
            }
            fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
                <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
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

text_enum! {
    /// A wormhole's remaining mass, worst last. Variant order is the severity order the
    /// [reconcile-on-link merge](../../docs/database/mapping.md) relies on: `max` is the
    /// worst (= "massed"). Kept in lock-step across a connection and its signatures by the
    /// `map_*_sync` DB triggers (migration 0009).
    pub enum MassStatus {
        Stable => "stable",
        Reduced => "reduced",
        Critical => "critical",
    }
}

text_enum! {
    /// A wormhole's remaining lifetime, worst last. `Eol` ≈ "<4h"; `Critical` ≈ "<1h"
    /// (super-EOL). Same severity-ordering / merge semantics as [`MassStatus`].
    pub enum TimeStatus {
        Stable => "stable",
        Eol => "eol",
        Critical => "critical",
    }
}

text_enum! {
    /// Max ship-mass class that can transit a wormhole. Ordered most-permissive →
    /// most-restrictive, so `max` (= `Small`) is the "weakest"/worst — the conservative
    /// pick when two ends disagree (they shouldn't: both ends of a hole share a size).
    pub enum WormholeSize {
        Xl => "xl",
        Large => "large",
        Medium => "medium",
        Small => "small",
    }
}

text_enum! {
    /// A cosmic-signature group, mirroring [`signature_categories`](../../docs/database/static.md).
    /// Only `Wormhole` signatures carry connection links and the wormhole life-cycle state.
    pub enum SignatureGroup {
        Wormhole => "wormhole",
        Data => "data",
        Relic => "relic",
        Gas => "gas",
        Combat => "combat",
        Ore => "ore",
        Unknown => "unknown",
    }
}

impl Default for SignatureGroup {
    /// An unresolved sig is `Unknown` until its group is scanned/classified.
    fn default() -> Self {
        Self::Unknown
    }
}

text_enum! {
    /// A placed system's intel status (`map_solar_system_details.status`), set by users.
    /// Matches the legacy vocabulary: `active` = recent activity seen, `empty` = scanned
    /// and found empty.
    pub enum SystemStatus {
        Unknown => "unknown",
        Friendly => "friendly",
        Hostile => "hostile",
        Active => "active",
        Unscanned => "unscanned",
        Empty => "empty",
    }
}

impl Default for SystemStatus {
    /// A freshly placed system is `Unscanned` until someone classifies it.
    fn default() -> Self {
        Self::Unscanned
    }
}

text_enum! {
    /// A wormhole system's kill-activity threat level, from the daily analysis.
    pub enum ThreatLevel {
        Unknown => "unknown",
        High => "high",
        Critical => "critical",
    }
}

/// A user acting as one of their characters. `user_id` drives authorization (effective
/// role across all their characters); `character_id` attributes ownership on creation.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Actor {
    pub user_id: i64,
    pub character_id: i64,
}

/// The graph as seen by a viewer: the map plus its placed systems and connections.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapView {
    pub map: Map,
    pub systems: Vec<MapSystemView>,
    pub connections: Vec<MapConnection>,
}

/// Map canvas geometry. Server-owned (built from env in `crate::config`), fetched by the
/// client so layout has a single source of truth. Node height is `2 * cell_size`;
/// dimensions are world-space px.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct GridConfig {
    pub cell_size: f64,
    pub world_width: f64,
    pub world_height: f64,
    pub viewport_height: f64,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            // Match the legacy map: 20-unit grid cells, so the node (2 cells) is 40 tall.
            cell_size: 20.0,
            world_width: 4000.0,
            world_height: 2000.0,
            viewport_height: 1400.0,
        }
    }
}
