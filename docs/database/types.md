# Item types

The shared item-type reference imported from the **SDE** — every ship, module, planet,
gate, structure, etc. is a *type*, grouped into groups and categories and (for items) a
market group, with per-type **attributes** (dogma). Many other tables reference these by
id (a ship in [`character_status`](./tracking.md#character_status); a gate/planet/station
type in [`universe`](./universe.md)). Part of the [database spec](./README.md).

Materialised from the SDE like the [universe](./universe.md) tables, and reloaded when
the SDE updates. Names are the SDE localized string; store the English value. Ids are the
SDE `_key`. Each table notes its SDE source file.

## Item hierarchy

### `categories`

*Source: SDE `categories.jsonl`.* The broadest grouping (Ship, Module, Celestial, …).

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `name` | text | |
| `published` | bool | |

### `groups`

*Source: SDE `groups.jsonl`.*

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `category_id` | fk categories | |
| `name` | text | |
| `published` | bool | |

### `market_groups`

*Source: SDE `marketGroups.jsonl`.* The market browser tree (a separate hierarchy from
category/group).

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `parent_group_id` | fk market_groups, null | tree parent; null at the root |
| `name` | text | |
| `has_types` | bool | whether items sit directly under it |

### `types`

*Source: SDE `types.jsonl`.* Every concrete item type — the most-referenced reference
table.

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `group_id` | fk groups | |
| `market_group_id` | fk market_groups, null | |
| `name` | text | |
| `published` | bool | |
| `volume` | double, null | |
| `mass` | double, null | hull mass — note: wormhole mass math also reads ship mass |
| `capacity` | double, null | |
| `icon_id` | int, null | for rendering icons |

**Invariants & expected behaviour**

- `category → group → type` is the canonical hierarchy; `market_group` is an
  independent tree for the market UI.
- Reference data: reloaded from the SDE, not written by the app.

## Type attributes (dogma)

Per-type attribute values (ship mass, signature radius, resistances, …) and their
definitions. Useful well beyond display — e.g. wormhole mass calculations read ship
attributes.

### `dogma_attributes`

*Source: SDE `dogmaAttributes.jsonl`.* The attribute catalogue (what each attribute
*is*).

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `name` | text | machine name (e.g. `mass`, `signatureRadius`) |
| `unit_id` | fk dogma_units, null | how to render the value |
| `default_value` | double | value when a type doesn't override it |
| `high_is_good` | bool | whether higher is better (for UI) |
| `published` | bool | |

### `dogma_units`

*Source: SDE `dogmaUnits.jsonl`.* Units an attribute value is expressed in (m, m/s, %, …).

| Column | Type | Notes |
|--------|------|-------|
| `id` | pk | |
| `name` | text | |

### `type_attributes`

*Source: SDE `typeDogma.jsonl`.* The per-type attribute values — a many-to-many between
`types` and `dogma_attributes` carrying the value.

| Column | Type | Notes |
|--------|------|-------|
| `type_id` | fk types | |
| `attribute_id` | fk dogma_attributes | |
| `value` | double | this type's value for the attribute |

**Invariants & expected behaviour**

- Primary key `(type_id, attribute_id)`; one value per attribute per type.
- An attribute absent for a type implies its `dogma_attributes.default_value`.
