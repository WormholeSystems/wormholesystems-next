# Custom static reference

Reference tables **seeded from the committed JSON in `data/static/`** — custom data the
legacy WormholeSystems app maintained that the SDE does **not** provide: the wormhole
type catalogue, system effects, J-space system data, the signature catalogue, and Jove
observatories. Distinct from the SDE-derived [universe](./universe.md) /
[types](./types.md) tables. Part of the [database spec](./README.md).

The `data/static/*.json` files are the **source of truth** (versioned in git); a seeder
upserts them into the tables below — idempotent, re-run when a file changes. Wormhole
class ids use the `wormhole_class_id` encoding (the `_encoding` map in each file; see
[universe](./universe.md)). System references resolve against
[`solar_systems`](./universe.md#solar_systems).

These tables also back the live map: a scanned wormhole
[`signature`](./mapping.md#signatures)'s type comes from `wormhole_types` (its mass /
lifetime), and `signature_categories` mirrors the signature `group`.

## `wormhole_types`

*Source: `wormholes.json`.* The catalogue of wormhole types (`K162`, `C247`, `T405`, …)
with their mass and lifetime limits.

| Column | Type | Notes |
|--------|------|-------|
| `code` | pk (text) | wormhole code, e.g. `C247` |
| `type_id` | int | SDE type id (`typeID`) |
| `dest_class` | int, null | destination [class](./universe.md), e.g. C5 = 5 |
| `is_static` | bool, null | tri-state; `null` for the generic `K162` |
| `max_mass_per_jump` | bigint, null | kg per jump |
| `total_mass` | bigint, null | kg total before collapse |
| `mass_regen` | bigint, null | kg regenerated |
| `lifetime_hours` | float, null | hours before natural decay (fractional — some are e.g. 4.5) |
| `sibling_groups` | jsonb, null | related-type groupings (rarely set) |
| `signature_strength` | float, null | scan signature strength in percent |

**Invariants & expected behaviour**

- The generic return hole `K162` has every limit `null` (only `type_id` is known) —
  its real properties come from the *other* side it connects to.
- `dest_class` and the source classes are `wormhole_class_id` values.

### `wormhole_type_sources`

*From `wormholes.json` `src[]`.* Which classes a wormhole type can spawn **in**.

| Column | Type | Notes |
|--------|------|-------|
| `wormhole_code` | fk wormhole_types | |
| `wormhole_class_id` | int | a source class |

Primary key `(wormhole_code, wormhole_class_id)`.

## `wormhole_effects`

*Source: `wormhole_effects.json`.* The six system effects.

| Column | Type | Notes |
|--------|------|-------|
| `name` | pk (text) | `Pulsar`, `Magnetar`, `Red Giant`, `Black Hole`, `Cataclysmic Variable`, `Wolf-Rayet Star` |

### `wormhole_effect_modifiers`

The per-class strength of each effect. Each effect has **Buffs** and **Debuffs**, each a
stat with six values (C1…C6).

| Column | Type | Notes |
|--------|------|-------|
| `effect_name` | fk wormhole_effects | |
| `kind` | enum | `buff`, `debuff` |
| `stat` | text | e.g. `Smart Bomb Damage` |
| `wormhole_class_id` | int | 1–6 |
| `value` | text | e.g. `+30%` (sign carried in the string) |

Unique `(effect_name, kind, stat, wormhole_class_id)`.

## `wormhole_systems`

*Source: `wormhole_systems.json`.* The WH-specific data the SDE lacks for J-space
systems. The system's `id` **is** an SDE solar system, so this augments
[`solar_systems`](./universe.md#solar_systems) rather than duplicating it (the source's
`name` / `constellation_id` / `region_id` are redundant SDE fields).

| Column | Type | Notes |
|--------|------|-------|
| `solar_system_id` | pk, fk solar_systems | the J-space system |
| `wormhole_class_id` | int | the system's class (C1–C6, 12–23) |
| `effect_name` | fk wormhole_effects, null | system effect, if any |
| `is_shattered` | bool | shattered system (106 systems, all classes; C13 is only the frigate subset) |

### `wormhole_system_statics`

*From `wormhole_systems.json` `statics[]`.* The static wormhole(s) a system always has.

| Column | Type | Notes |
|--------|------|-------|
| `solar_system_id` | fk wormhole_systems | |
| `wormhole_code` | fk wormhole_types | the static's type |

Primary key `(solar_system_id, wormhole_code)`.

## `signature_categories`

*Source: `signatures.json` `categories`.* The cosmic-signature groups; `code` mirrors
the [`signatures.group`](./mapping.md#signatures) enum.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `name` | text | e.g. `Wormhole`, `Data Site` |
| `code` | text, unique | `wormhole`, `data`, `relic`, `combat`, `gas`, `ore`, `homefront` |

## `signature_types`

*Source: `signatures.json` `types`.* The known signature types — for wormholes, the
`signature` code matches a [`wormhole_types`](#wormhole_types) `code`.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `signature` | text, null | the code, e.g. `B274`; null for non-wormhole sites (data/anomaly) |
| `name` | text | e.g. `B274 - H` |
| `signature_category_id` | fk signature_categories | |
| `target_class` | int, null | where it leads (`wormhole_class_id`) |
| `extra` | text, null | extra label, e.g. drifter name `Barbican` |

### `signature_type_spawn_areas`

*From `signatures.json` `spawn_areas[]`.* Classes a signature type can spawn in.

| Column | Type | Notes |
|--------|------|-------|
| `signature_type_id` | fk signature_types | |
| `wormhole_class_id` | int | a spawn class |

Primary key `(signature_type_id, wormhole_class_id)`.

## `jove_observatories`

*Source: `jove_observatories.json`.* Systems that contain a Jove Observatory (~1019).
The source is keyed region → system **names**; the seeder resolves names to
[`solar_systems`](./universe.md#solar_systems) ids. Presence of a row = has an
observatory.

| Column | Type | Notes |
|--------|------|-------|
| `solar_system_id` | pk, fk solar_systems | |

**Invariants & expected behaviour**

- Presence-only: a row means the system has a Jove Observatory; no row means it doesn't.
