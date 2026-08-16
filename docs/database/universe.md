# Universe

The EVE universe the map renders on: the static topology imported from the **SDE**
(regions → constellations → systems → planets/moons/stargates/stations, plus factions),
the **dynamic** sovereignty overlay from ESI, and player **structures** discovered via
ESI. Part of the [database spec](./README.md).

We materialise a *subset* of the SDE (see `src/sde/`) into Postgres so the app can join
against it — a placed system's name and security, stargate adjacency for routing, who
holds a system. The SDE-backed tables are **reference data**, reloaded when the SDE is
updated (see [Seeding](./seeding.md)); the ESI-backed tables (`structures`,
`system_sovereignty`) are dynamic.

Each table notes its SDE source file. Names in the SDE are a localized string; store the
English value (or a `jsonb` of all locales if we later localize). Ids are the SDE `_key`
unless noted.

## SDE topology

### `regions`

*Source: SDE `mapRegions.jsonl`.*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `name` | text | |
| `faction_id` | int, null | sovereign/owning faction, if any |
| `wormhole_class_id` | int, null | set for j-space / special regions |

### `constellations`

*Source: SDE `mapConstellations.jsonl`.*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `region_id` | fk regions | |
| `name` | text | |
| `faction_id` | int, null | |

### `solar_systems`

*Source: SDE `mapSolarSystems.jsonl`.* The core node. A
[`map_solar_systems`](./mapping.md#map_solar_systems) placement references one of these
by `solar_system_id`.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `constellation_id` | fk constellations | |
| `region_id` | fk regions | denormalised for convenient joins |
| `name` | text | |
| `security_status` | double | −1.0 … 1.0 (drives hi/low/null colouring) |
| `security_class` | text, null | |
| `faction_id` | int, null | |
| `wormhole_class_id` | int, null | effective EVE class id (system value, else wormhole-system catalogue, else region; filled by the seed post-pass) |
| `star_id` | int, null | |

> **Class resolution.** The SDE sets `wormholeClassID` mostly at the **region** level
> (highsec = 7, nullsec = 9, C1–C6, Thera = 12, abyssal = 19–23, Pochven = 25); only
> **lowsec = 8** and the drifter hubs (14–18) are stamped per-system. Store the
> *effective* class here: the system's own value if present, else its region's.

J-space systems carry extra WH-only data (effect, statics) the SDE lacks — see
[`wormhole_systems`](./static.md#wormhole_systems) in the custom static reference.

### `stargates`

*Source: SDE `mapStargates.jsonl`.* The **real** NPC gate topology — the K-space
adjacency graph used for routing (shortest-path-home). Distinct from the user-drawn
[`map_connections`](./mapping.md#map_connections): those are a player's chain, these are
the fixed gate network.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `solar_system_id` | fk solar_systems | the system the gate sits in |
| `destination_system_id` | fk solar_systems | system on the other side |
| `destination_stargate_id` | int | the paired gate |
| `type_id` | int | gate type (→ [`types`](./types.md)) |

Each gate is one direction; its paired gate provides the reverse edge.

### `planets`

*Source: SDE `mapPlanets.jsonl`.*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `solar_system_id` | fk solar_systems | |
| `type_id` | int | planet type (→ [`types`](./types.md)) |
| `celestial_index` | int | the "IV" in "Jita IV" |
| `name` | text, null | SDE `unique_name`, when present |

### `moons`

*Source: SDE `mapMoons.jsonl`.*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `solar_system_id` | fk solar_systems | |
| `type_id` | int | |
| `celestial_index` | int | |
| `name` | text, null | SDE `unique_name`, when present |

### `asteroid_belts`

*Source: SDE `mapAsteroidBelts.jsonl`.*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `solar_system_id` | fk solar_systems | |
| `type_id` | int | belt type |
| `celestial_index` | int | |
| `name` | text, null | SDE `unique_name`, when present |

### `stations`

*Source: SDE `npcStations.jsonl`.* NPC stations. The SDE row has **no name** (it's
generated from operation + celestial), so `name` is resolved from ESI
`GET /universe/stations/{id}` when needed. A docked character's `station_id`
([`character_status`](./tracking.md#character_status)) matches one of these.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `solar_system_id` | fk solar_systems | |
| `type_id` | int | station type |
| `owner_corporation_id` | int, null | NPC corp that owns it (SDE `ownerID`) |
| `operation_id` | int, null | |
| `name` | text, null | resolved via ESI |

### `factions`

*Source: SDE `factions.jsonl`.* Needed for display and as sovereignty/space holders,
even though factions never authenticate.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `name` | text | |
| `description` | text, null | |
| `corporation_id` | int, null | |
| `militia_corporation_id` | int, null | faction-warfare militia |
| `home_solar_system_id` | int, null | SDE `solarSystemID` |
| `size_factor` | double, null | |

## Entities (ESI-cached)

`corporations` and `alliances` are resolved from ESI (during login / affiliation
refresh) and cached for display — names, tickers, and the ids that link them. Unlike
the SDE topology these change over time, so they carry an `updated_at`.

### `corporations`

*Source: ESI [`GET /corporations/{id}`](../esi/corporation-public.md).*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk (bigint) | |
| `name` | text | |
| `ticker` | text | |
| `alliance_id` | bigint, null | current alliance |
| `faction_id` | bigint, null | |
| `ceo_id` | bigint, null | |
| `member_count` | int, null | |
| `updated_at` | timestamptz | last resolved |

### `alliances`

*Source: ESI [`GET /alliances/{id}`](../esi/alliance-public.md).*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk (bigint) | |
| `name` | text | |
| `ticker` | text | |
| `creator_corporation_id` | bigint, null | |
| `executor_corporation_id` | bigint, null | |
| `faction_id` | bigint, null | |
| `updated_at` | timestamptz | last resolved |

## Dynamic overlays

### `structures`

*Source: ESI `GET /universe/structures/{id}`, on demand.* Player-owned structures
(Upwell). **Not in the SDE.** Populated sparsely as we encounter them — e.g. a character
docked in a structure
([`character_status`](./tracking.md#character_status)`.structure_id`). Resolving a
structure requires docking access.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk (bigint) | structure id |
| `solar_system_id` | fk solar_systems, null | once known |
| `name` | text, null | |
| `type_id` | int, null | |
| `owner_corporation_id` | bigint, null | |
| `updated_at` | timestamptz | last resolved |

**Invariants & expected behaviour**

- Sparse: a row exists only for structures we've had reason to resolve; absence is
  normal (access-gated, may be unresolvable).

### `system_sovereignty`

*Source: ESI [`GET /sovereignty/systems`](../esi/sovereignty-systems.md).* Who currently
holds each K-space system — **dynamic**, refreshed by
[sovereignty refresh](../processes.md#sovereignty-refresh). The ESI claim is one of
**faction**, **alliance**, or **unclaimed**.

| Column | Type | Notes |
|--------|------|-------|
| `solar_system_id` | pk, fk solar_systems | one row per system |
| `alliance_id` | bigint, null | holding alliance (alliance claim) |
| `corporation_id` | bigint, null | holding corporation (alliance claim) |
| `faction_id` | int, null | holding faction (faction claim) |
| `claimed_since` | timestamptz, null | from an alliance claim |
| `is_capital_system` | bool, null | alliance's capital system |
| `updated_at` | timestamptz | last refresh |

**Invariants & expected behaviour**

- One row per claimed system; a system is held by **either** an alliance (with its
  corporation) **or** a faction — not both. Unclaimed systems have no row (or all-null).
- Refreshed wholesale each run; a system that flips owner reflects it after the next
  refresh.
- Displayed on the map to show who owns a system.
