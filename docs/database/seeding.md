# Seeding

How the reference tables are populated and kept current. The seeder lives in
`src/seed/`; run it manually with `cargo run -- seed`, or let it run automatically at
startup (see [Build gate](#build-gate)).

The seeder is the **write path for reference data only** — the SDE-derived tables
([Item types](./types.md), [Universe](./universe.md) topology) and the
[custom static reference](./static.md). It never touches runtime data (users,
characters, ESI tokens, maps, signatures, live status), which is owned by the app and
ESI.

## Sources

- **SDE** — the `.jsonl` files unpacked under `data/sde/`, loaded via `src/sde/`.
  Categories, groups, market groups, types, the dogma catalogue and type attributes,
  factions, NPC corporations, and the universe topology (regions → constellations →
  solar systems, plus stargates, planets, moons, asteroid belts, NPC stations).
- **Custom static** — the hand-authored JSON in `data/static/` (wormhole types/systems/
  effects, the signature catalogue, Jove observatories). See [static.md](./static.md).

## One transaction

The whole seed runs in a **single transaction**. This is what lets the
`factions ⇄ corporations` cycle load: those cross-references are `DEFERRABLE INITIALLY
DEFERRED` (see [universe.md](./universe.md)) and validate at commit, after both sides
are present. The same trick covers the **self-referential** `market_groups.parent_group_id`
— children can be inserted before their parent within the transaction, so the chunked
bulk insert needs no topological ordering.

Inline (non-deferred) FKs — a celestial's `type_id`, a constellation's `region_id` — are
satisfied by **insert order**: types and the universe parents are written before the
rows that reference them.

## Upsert, not insert-only

> **Decision.** Entity tables **upsert** (`on conflict (id) do update set …`); the SDE
> is the source of truth and a re-seed must *correct* drifted rows (a renamed type, a
> moved system), not just add new ones.

- **Entity tables** (categories, groups, market_groups, types, dogma_*, type_attributes,
  factions, corporations, regions, constellations, solar_systems, stargates, planets,
  moons, asteroid_belts, stations) upsert every SDE-owned column.
- **`corporations` is shared** with ESI (player corps). The seeder updates only the
  SDE-owned columns (`name`, `ticker`, `faction_id`, `ceo_id`) on conflict and leaves
  `alliance_id` / `member_count` — which ESI maintains — untouched.
- **Link / junction tables** whose primary key *is* the whole row (wormhole sources and
  statics, effect modifiers, signature spawn areas, jove observatories) use `on conflict
  do nothing`: there is nothing to update.

> **Known limitation.** Upsert and `do nothing` are both *additive* — a row the SDE
> *removes* is not deleted by a re-seed. Removals are rare for this data; to fully
> reconcile, re-seed into a fresh database. Revisit with per-parent delete+reinsert if a
> table proves churny.

`stations.owner_corporation_id` references a (deferred) corporation. Owners are NPC
corps; any owner not present in the seeded corporation set is stored as `null` so the FK
still validates at commit.

## Build gate

> **Decision.** Seeding kicks off at **app startup**, but only when needed — gated on
> the SDE **build number**, not run unconditionally. A full re-seed writes ~1M rows
> (moons alone are ~340k); doing that every boot would be pointless when nothing changed.

`data/sde/_sde.jsonl` carries the build of the unpacked SDE:

```json
{"_key": "sde", "buildNumber": 3409592, "releaseDate": "2026-06-25T12:00:48Z"}
```

The `sde_build` table records the build currently loaded (a single-row table — boolean
primary key defaulting to `true`, `check (id)`, so a second row is impossible). At
startup `seed::ensure_seeded`:

1. reads the bundled build from `_sde.jsonl`;
2. compares it to `sde_build.build_number`;
3. seeds (and records the new build, in the same transaction) only if the bundled build
   differs or no build is loaded yet; otherwise returns immediately.

So the steady state is one cheap query per boot; the first boot, or a boot after the
bundled SDE is bumped, pays for a re-seed. `cargo run -- seed` ignores the gate and
always re-seeds, for deliberate manual reloads.

**Invariants & expected behaviour**

- A second consecutive seed is a no-op for row counts (idempotent upsert).
- After a successful seed, `sde_build` holds exactly one row, equal to `_sde.jsonl`'s
  build number.
- `ensure_seeded` does no writes when the loaded build matches the bundled build.

## Staying current with CCP

Two distinct freshness questions, kept separate:

1. **Is the DB in sync with the SDE files I have?** — the local [build gate](#build-gate)
   above. Offline, deterministic, runs at boot.
2. **Has CCP published a newer SDE than the one in `data/sde`?** — a network check.
   `sde::download::latest_build()` returns CCP's current build number; comparing it to
   `_sde.jsonl` tells us whether a newer SDE exists.

> **Decision (planned).** The CCP-freshness check is surfaced as an **admin prompt in
> the UI**, not an interactive startup prompt and not an automatic download. Boot stays
> offline and fast; a background task periodically asks CCP for the latest build, and
> when it's newer than what we have, the app shows an "SDE update available" banner to an
> admin, who triggers download + re-seed. This avoids blocking startup on a network call
> or a large download.
>
> **Status:** the build comparison primitives exist (`latest_build()` + `_sde.jsonl`);
> the background task, the admin role, and the UI banner are **not yet built**.
