# Mapping

The live map graph: solar systems placed on a map and the wormhole / stargate
connections between them. Part of the [database spec](./README.md) — see it for the
conventions and goals these tables serve.

## `maps`

The top-level artifact. A user creates a map and everything else hangs off it.

| Column        | Type        | Notes                                                       |
|---------------|-------------|-------------------------------------------------------------|
| `id`          | pk          |                                                             |
| `name`        | text        | user-chosen                                                 |
| `description` | text, null  | free-text, user-set                                         |
| `image_url`   | text, null  | reference to an uploaded icon/logo, **not** the image bytes |
| `created_at`  | timestamptz |                                                             |

**Invariants & expected behaviour**

- A map always has at least one **owner**, recorded as a `role = owner` row in
  [`map_access`](./access.md#map_access) (there is no `owner_id` column).
- On creation, the map gets an `owner` access grant for its creator's character.
- The icon/logo is **uploaded to object storage** (S3-style or a static asset dir);
  `image_url` holds only the reference. We do not store image bytes in Postgres.
- Deleting a map cascades to its solar systems, connections, signatures, details,
  and access entries (nothing dangles). Removing the map should also clean up its
  uploaded image from object storage (app-level, outside the DB transaction).

---

## `map_solar_systems`

*Ephemeral placement.* A solar system *as currently placed on a map*. Holds only data that is meaningful
while the system is on the map; removing the system deletes this row.

| Column            | Type        | Notes                                            |
|-------------------|-------------|--------------------------------------------------|
| `id`              | pk          |                                                  |
| `map_id`          | fk maps     |                                                  |
| `solar_system_id` | int         | → [`solar_systems`](./universe.md#solar_systems) |
| `position_x`      | int/float   | grid position on the map                         |
| `position_y`      | int/float   | grid position on the map                         |
| `alias`           | text, null  | temporary, user-set; lost on removal             |
| `created_at`      | timestamptz |                                                  |

**Invariants & expected behaviour**

- Unique `(map_id, solar_system_id)` — a system appears at most once per map.
- A system always has an `(x, y)` position while placed.
- The alias is **ephemeral**: removing the system from the map and re-adding it does
  **not** restore the previous alias.
- Removing a system deletes its placement, its **signatures**, and its connections —
  but **not** its persisted details (next table).

---

## `map_solar_system_details`

*Persisted intel.* Per-`(map, system)` intel that must **survive** the system being removed from the
map and be shown again when it is re-added. Lives independently of
`map_solar_systems`.

| Column             | Type        | Notes                                                    |
|--------------------|-------------|----------------------------------------------------------|
| `id`               | pk          |                                                          |
| `map_id`           | fk maps     |                                                          |
| `solar_system_id`  | int         | SDE `_key`                                               |
| `status`           | enum        | e.g. `unscanned`, `scanned`, `friendly`, `hostile`       |
| `occupying_group`  | text, null  | who holds the system (intel)                             |
| `updated_at`       | timestamptz |                                                          |

**Invariants & expected behaviour**

- Unique `(map_id, solar_system_id)`.
- A details row may exist with **no** corresponding `map_solar_systems` row (system
  not currently placed) — that is the whole point.
- Round-trip: add system → set status/occupying group → remove → re-add ⇒ the status
  and occupying group are still there. (Signatures, by contrast, are gone.)
- Deleting the **map** removes its details; removing a single **system** does not.

> **Open — status values.** Final enum set (`unscanned`/`scanned`/`friendly`/
> `hostile`/…?). And is `occupying_group` free text or a reference to a known entity?

---

## `signatures`

A cosmic signature in a system, as scanned from the in-game probe scanner. For a
**wormhole** signature this is also where the hole's life-cycle state lives, because
that state is visible from the scan *before* the hole is jumped or connected. The
catalogue of signature groups and known wormhole types is
[custom static reference](./static.md).

| Column            | Type             | Notes                                                       |
|-------------------|------------------|-------------------------------------------------------------|
| `id`              | pk               |                                                             |
| `map_id`          | fk maps          |                                                             |
| `solar_system_id` | int              | SDE `_key` — the system the sig is in                       |
| `signature_id`    | text             | in-game id, e.g. `ABC-123`                                  |
| `group`           | enum             | `wormhole`, `data`, `relic`, `gas`, `combat`, `ore`, `unknown` |
| `name`            | text, null       | resolved wormhole type / site name when known               |
| `size`            | enum, null       | `xl`, `large`, `medium`, `small` (wormholes)                |
| `mass_status`     | enum, null       | `stable`, `reduced`, `critical` ("massed")                  |
| `time_status`     | enum, null       | `stable`, `eol`, `critical` (≈ < 1h, "super EOL")           |
| `connection_id`   | fk map_connections, null | set when this sig is linked as one end of a hole      |
| `created_at`      | timestamptz      |                                                             |
| `updated_at`      | timestamptz      |                                                             |

**Invariants & expected behaviour**

- Unique `(map_id, solar_system_id, signature_id)`.
- A `group = wormhole` signature **may carry `size` / `mass_status` / `time_status`
  with no `connection_id`** — the scanned-but-not-yet-jumped case. This is required.
- Only a `group = wormhole` signature may have a `connection_id`; the wormhole state
  columns are `null`/ignored for non-wormhole groups.
- **Ephemeral:** when a system is removed from the map, its signatures are deleted
  along with the placement (scan data goes stale; we do not persist it).
- Paste-from-scanner reconciliation (future spec): a paste adds new sigs, removes
  sigs no longer present, and preserves connection assignments where the id matches.

### Keeping a connection's two signatures consistent

A jumped wormhole appears as **two signatures** — one in each system — that describe
the *same* hole, linked through a `map_connections` row. Their shared state (`size`,
`mass_status`, `time_status`) must agree.

Recommended approach: **state lives on the signature; a PostgreSQL trigger keeps the
two signatures of a connection in lock-step.** Editing `mass_status` (or size/time)
on either end propagates to the sibling end. We use a trigger rather than app-side
code so the two ends can never drift regardless of which code path writes them, and
because the state legitimately exists on the signature *before* a connection does
(so "single source of truth on the connection" isn't available to us).

The relationship itself — at most two signatures per connection, one per endpoint —
is enforced as an invariant/constraint, not synchronized data.

> **Open — reconcile on link.** When two *already-scanned* signatures with differing
> state are linked into one connection, which side wins (most-recently-updated? the
> side initiating the link?) before the trigger equalises them?

---

## `map_connections`

An edge between two placed systems. A wormhole edge is backed by the two signatures
above; a stargate edge is just a known gate.

| Column          | Type                 | Notes                              |
|-----------------|----------------------|------------------------------------|
| `id`            | pk                   |                                    |
| `map_id`        | fk maps              |                                    |
| `from_system`   | fk map_solar_systems |                                    |
| `to_system`     | fk map_solar_systems |                                    |
| `type`          | enum                 | `wormhole`, `stargate`             |
| `created_at`    | timestamptz          |                                    |

**Invariants & expected behaviour**

- Connects two **distinct** systems in the **same** map.
- The wormhole life-cycle state is **not** stored here — it lives on the linked
  `signatures`. A connection's displayed size/mass/time is read from them.
- A `wormhole` connection is backed by up to two signatures (one in `from_system`,
  one in `to_system`); a `stargate` connection has no signatures.
- Removing either endpoint system deletes the connection; deleting the connection
  clears `connection_id` on its signatures (they survive as plain wormhole sigs while
  their system is still placed).

> **Open — vocabulary.** Confirming the mapping from your words: "massed" →
> `mass_status = reduced`/`critical`; "EOL" → `time_status = eol`; "super EOL,
> < 1 hour" → `time_status = critical`. And `size` = max-jumpable mass class
> (S/M/L/XL)?
