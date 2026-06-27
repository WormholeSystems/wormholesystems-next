# Map actions

The **business logic** for maps: the actions a user can take on a map and exactly how
each behaves. This is the application layer above the [database spec](../database/) —
it turns the tables ([mapping](../database/mapping.md), [access](../database/access.md))
into a set of authorized, validated operations.

Scope of this doc: **action-based, no UI.** Each action is a server-side function with a
clear signature, authorization rule, validation, effect, and error set — the contract
tests are derived from. UI / HTTP wiring (Leptos server functions, Axum routes) comes
later and just calls these.

In scope: map lifecycle (create / rename / delete / list / get), access grants, and
graph editing (add / remove / move systems, add / remove connections). **Out of scope
for now:** signatures and the scanner-paste reconciliation (own spec, has open
questions and a DB trigger — see [mapping.md](../database/mapping.md#signatures)), live
tracking, and routing.

## The actor

Every action is taken by a **user** acting **as one of their characters** (the
per-session active character — see [authentication](../database/authentication.md)).

```
Actor { user_id, character_id }
```

- `user_id` drives **authorization**: a user's effective role is the highest role they
  match across *all* their characters (a character's own id, its `corporation_id`, or
  its `alliance_id` matching a `map_access.subject_id`). See
  [access.md](../database/access.md).
- `character_id` is recorded where an action attributes ownership to a *character* (map
  creation grants ownership to the acting character, not the user).

`character_id` must belong to `user_id` — actions validate this and reject otherwise
(`Forbidden`).

## Action shape: command in, validation split

Every mutating action takes a **dedicated command struct** as its argument, with `actor`
as a *separate* parameter:

```
add_connection(pool, actor, AddConnection { map_id, from_system, to_system, kind })
```

- Named fields kill the "transposed positional argument" bug — several actions take
  multiple same-typed `i64`s (`map_id`, `from_system`, `to_system`) that are otherwise
  easy to swap.
- The command struct *is* the future HTTP/server-function request body (it derives
  `Serialize`/`Deserialize`). `actor` stays separate so it is always injected from the
  authenticated session — a client can never set its own `user_id`.

**Validation is split by what it needs:**

- **Pure / context-free** checks (non-blank name, `from != to`) live on the command via a
  `Validate` trait the action calls first. They depend only on the input, so the **UI can
  reuse them** for pre-submit feedback.
- **Stateful / referential** checks (row exists, not already placed, both endpoints on
  *this* map, last-owner, character-belongs-to-user) **stay in the action**. They depend
  on DB state that can change between check and write, so they must be atomic with it;
  pulling them out would reintroduce a time-of-check/time-of-use race and let callers
  bypass invariants. Where the schema already enforces a rule (unique indexes, FKs, the
  `from <> to` CHECK), the action leans on it and maps the error to `MapError`.

So an action body reads: **validate input → authorize → perform (constraint-backed) → map
errors**, and stays authoritative for correctness.

## Roles

`Role` is an ordered enum, `Viewer < Member < Manager < Owner` (stored as text per the
[enum convention](../database/README.md)). An action states the **minimum** role it
requires; a user passes if their effective role is `>=` it.

| Action group        | Minimum role |
|---------------------|--------------|
| View / list / get   | Viewer       |
| Edit the graph (systems, connections) | Member |
| Manage access (grant / revoke / set role) | Manager |
| Modify the map (rename, settings, delete) | Owner |

This mirrors the capability table in [access.md](../database/access.md#roles--capabilities).

### Effective-role resolution

`effective_role(map_id, user_id) -> Option<Role>`:

1. Collect the user's characters and each one's `(id, corporation_id, alliance_id)`.
2. Gather the EVE ids into one set.
3. Return the **max** `role` over `map_access` rows for this map whose `subject_id` is
   in that set, or `None` if no row matches.

`None` means "no access at all" and is reported as **`NotFound`** (we do not reveal that
a map the user can't see exists). Having *some* access but too low a role for the action
is **`Forbidden`**.

## Error model

A single `MapError` (thiserror) is returned by every action:

| Variant               | When                                                              |
|-----------------------|------------------------------------------------------------------|
| `NotFound`            | Map (or referenced row) doesn't exist, or the user has no access to it |
| `Forbidden`          | User has access but a lower role than the action requires; or acting as a character that isn't theirs |
| `Conflict`           | Uniqueness / idempotency violation (system already placed, connection already exists) |
| `Validation(String)` | Bad input (blank name, self-connection, endpoint not on the map, system id unknown) |
| `LastOwner`          | The operation would leave the map with zero owners                |
| `Db(sqlx::Error)`    | Underlying database error                                         |

## Transactions

Any action with more than one write runs in a single transaction, so it is all-or-
nothing: map creation (insert map **+** owner grant), system removal (the placement and
its cascaded connections/signatures), role changes that must re-check the owner
invariant. Single-statement actions don't need an explicit transaction.

---

## Map lifecycle

### `create_map(actor, name, description?) -> Map`

- **Auth:** any authenticated user.
- **Validates:** `name` is trimmed and non-empty; `character_id` belongs to `user_id`.
- **Effect (one transaction):** insert the `maps` row, then insert a `map_access` row
  `(subject_type = character, subject_id = actor.character_id, role = owner)`.
- **Invariants:**
  - The new map has exactly one access row, an `owner` for the acting character.
  - A blank/whitespace name is rejected with `Validation`.

### `update_map(actor, map_id, { name?, description?, image_url? }) -> Map`

- **Auth:** `Owner`.
- **Validates:** if `name` is given it must be non-empty after trimming. Omitted fields
  are left unchanged; `description` / `image_url` can be explicitly set to `null`.
- **Effect:** updates the `maps` row.
- **Note:** swapping `image_url` does not touch object storage here; the old image is
  cleaned up by the app outside this action (see
  [maps invariants](../database/mapping.md#maps)).

### `delete_map(actor, map_id)`

- **Auth:** `Owner`.
- **Effect:** deletes the `maps` row; the DB cascades placements, details, connections,
  signatures, and access grants. Object-storage image cleanup is app-level, outside this
  transaction.
- **Invariant:** after deletion no rows in any child table reference `map_id`.

### `list_maps(user_id) -> Vec<(Map, Role)>`

- **Auth:** any authenticated user; returns only maps the user can access.
- **Effect:** read-only. Each map is paired with the user's effective role on it.
- **Invariant:** a map appears iff `effective_role` is `Some`.

### `get_map(actor, map_id) -> MapView`

- **Auth:** `Viewer`.
- **Effect:** read-only; returns the map plus its placed systems and connections (the
  graph). Live pilot locations are **not** included here (that's the member-gated
  tracking path).

---

## Access management

### `set_access(actor, map_id, subject_type, subject_id, role)`

- **Auth:** `Manager`.
- **Privilege ceiling:** an actor may not grant a role **higher than their own**
  effective role. So a `Manager` can grant up to `Manager`; only an `Owner` can grant
  `Owner`. Violation → `Forbidden`.
- **Effect:** upsert the grant for `(map_id, subject_id)` (unique per
  [access.md](../database/access.md#map_access)); an existing subject's role is changed
  in place.
- **Invariants:**
  - `subject_id` is unique per map; re-granting the same subject updates its role.
  - Downgrading or changing the **last remaining owner** such that no owner is left is
    rejected with `LastOwner`.

### `revoke_access(actor, map_id, subject_id)`

- **Auth:** `Manager`.
- **Effect:** delete the grant.
- **Invariant:** revoking the **last owner** is rejected with `LastOwner` — every map
  always has at least one owner.

> **Open — owner constraints** (carried from [access.md](../database/access.md)): should
> `owner` / `manager` be restricted to `subject_type = character`? Single owner per map,
> or co-owners allowed? This spec assumes **multiple owners allowed, any subject type**,
> enforcing only "≥ 1 owner". Tightening later is additive.

---

## Graph editing

### `add_system(actor, map_id, solar_system_id, x, y, alias?) -> MapSolarSystem`

- **Auth:** `Member`.
- **Validates:** `solar_system_id` exists in [`solar_systems`](../database/universe.md);
  the system is not already placed on this map.
- **Effect:** insert a `map_solar_systems` row at `(x, y)`.
- **Invariants:**
  - Unique `(map_id, solar_system_id)` — a second add of the same system → `Conflict`.
  - Adding a system **does not** create or reset its
    [`map_solar_system_details`](../database/mapping.md#map_solar_system_details); if a
    details row already exists (system was placed before), it is left intact and resurfaces.
  - An unknown `solar_system_id` → `Validation`.

### `remove_system(actor, map_id, map_solar_system_id)`

- **Auth:** `Member`.
- **Effect:** delete the `map_solar_systems` row. The DB cascades the system's
  **signatures** and any **connections** it is an endpoint of. Its **details persist**.
- **Invariants:**
  - The placement, its signatures, and its connections are gone.
  - Its `map_solar_system_details` row (if any) still exists — round-trips on re-add.
  - Removing a system on a different map, or an id not on this map → `NotFound`.

### `move_system(actor, map_id, map_solar_system_id, x, y)`

- **Auth:** `Member`.
- **Effect:** update the placement's `(position_x, position_y)`.

### `set_alias(actor, map_id, map_solar_system_id, alias?)`

- **Auth:** `Member`.
- **Effect:** set or clear the **ephemeral** alias on the placement.
- **Invariant:** the alias is not persisted across removal (it lives on
  `map_solar_systems`, which is deleted on remove).

### `add_connection(actor, map_id, from_mss_id, to_mss_id, type) -> MapConnection`

`from_mss_id` / `to_mss_id` are `map_solar_systems` ids (placements), not SDE system ids.

- **Auth:** `Member`.
- **Validates:**
  - both placements exist and belong to **this** map;
  - they are **distinct** (`from <> to`);
  - no connection already exists between them (treated as **unordered** — A→B and B→A
    are the same edge);
  - `type` is `wormhole` or `stargate`.
- **Effect:** insert a `map_connections` row.
- **Invariants:**
  - A self-connection → `Validation`; an endpoint not on the map → `Validation`.
  - A duplicate edge (either direction) → `Conflict`.
  - No signature state is written here; a `wormhole` connection's size/mass/time come
    from its linked signatures (separate spec).

### `remove_connection(actor, map_id, connection_id)`

- **Auth:** `Member`.
- **Effect:** delete the `map_connections` row. Linked signatures survive with their
  `connection_id` cleared (DB `on delete set null`).
- **Invariant:** a connection id not on this map → `NotFound`.

---

## Testing approach

Every action has DB-backed tests using `#[sqlx::test]`, which provisions an **isolated
database per test** and runs the migrations. Each test seeds only the minimal fixtures
it needs — a user, one or more characters with affiliations, and a handful of
`solar_systems` rows — then drives actions and asserts on both the return value and the
resulting rows.

The tests are split by area, each its own integration-test binary, with shared fixtures
in `tests/common/`:

- `tests/maps_lifecycle.rs` — create / update / delete / list / get
- `tests/maps_access.rs` — grant / change role / revoke + the owner invariant
- `tests/maps_graph.rs` — systems and connections

Coverage targets, per action: the **happy path** asserting the exact effect (return-value
fields *and* the resulting rows match the inputs), each **authorization** boundary (role
just-too-low rejected, minimum role accepted), and each **invariant/error** bullet above
(e.g. duplicate add → `Conflict`, last-owner revoke → `LastOwner`, system round-trip
preserves details). The bullet lists in this doc are the test checklist.
