# Database

The application data model for `wormholesystems`, split by domain. The backing store is
**PostgreSQL**.

Written **spec first**: each doc describes the goals, structure, and expected
behaviour of its tables *before* implementation, so we can derive tests from the
invariants and measure the implementation against them.

## Domains

- **[Mapping](./mapping.md)** — `maps`, `map_solar_systems`,
  `map_solar_system_details`, `signatures`, `map_connections`. The live graph: systems
  placed on a map and the wormhole / stargate connections between them.
- **[Authentication](./authentication.md)** — `users`, `characters`, `tokens`,
  `scopes`, `token_scopes`, `oauth_login_flows`. EVE SSO identity and ESI tokens.
- **[Access](./access.md)** — `map_access` and the role / capability model. Who can
  see and do what on a map.
- **[Tracking](./tracking.md)** — `character_status`. Live ESI presence (location,
  online, ship), shown to map members and above.
- **[Universe](./universe.md)** — `regions`, `constellations`, `solar_systems`,
  `stargates`, `planets`, `moons`, `asteroid_belts`, `stations`, `factions` (SDE
  topology), plus `structures` and `system_sovereignty` (ESI overlays).
- **[Item types](./types.md)** — `types`, `groups`, `categories`, `market_groups`, and
  type attributes (`dogma_attributes`, `dogma_units`, `type_attributes`) — the SDE
  item-type reference, referenced by id across the schema.
- **[Custom static reference](./static.md)** — `wormhole_types`, `wormhole_effects`,
  `wormhole_systems`, `signature_categories`/`signature_types`, `jove_observatories`:
  data the SDE lacks, seeded from `data/static/`.
- **[Seeding](./seeding.md)** — how the reference tables (SDE + custom static) are
  populated and kept current: one transaction, upsert, and the `sde_build` startup gate.

## Conventions

- Tables are `snake_case` plural; primary keys are `id` unless noted.
- **Invariants & expected behaviour** bullets are the testable contract — each one
  should become at least one test.
- Blockquotes marked **Open** are decisions still to be confirmed, not settled facts.

## Goals

- **Maps are the core artifact.** A user creates a map and builds a live graph on it:
  solar systems are the nodes, wormhole or stargate **connections** are the edges.
- **Two tiers of per-system persistence.** Some data is *ephemeral* — it exists only
  while a system is placed on the map (its position, a temporary alias, its scanned
  signatures) and is gone when the system is removed. Other data is *persisted intel*
  — status, who occupies the system — which must survive removal and reappear when
  the system is added back to the same map later.
- **Wormhole life-cycle state lives on the signature.** Size class, mass status, and
  lifetime/EOL are observable from a *single scanned signature* before the hole has
  ever been jumped — so they must be storable with no connection in existence yet.
- **A connection links the two signatures of one wormhole.** Once both ends are known
  and the hole is placed on the map, the two signatures (one per system) describe the
  same physical wormhole and are kept consistent (see [Keeping a connection's two
  signatures consistent](./mapping.md#keeping-a-connections-two-signatures-consistent)).
- **Users own many characters.** Identity is per character; a user is a bag of
  characters. The *active* character is **per-session** (different devices can differ),
  seeded from the user's *preferred* character — it is not stored on the user.
- **Flexible, mixable, tiered access.** A map grants access to any combination of
  individual characters, corporations, and alliances by EVE ID, each at a role
  (viewer / member / manager / owner); the creator is recorded as the map's first
  owner grant. A user is authorized if any of their characters — or that character's
  corporation or alliance — appears in the map's access list.
- **Live presence.** Members and above see where each tracked character is, their
  ship, and whether they're online — sourced by polling ESI, gated by the viewer line.
- **Universe & reference data.** The map renders on SDE-derived topology (regions →
  systems → gates), with item types and factions as shared reference; a dynamic
  sovereignty overlay shows who currently owns each system.
- **Custom static reference.** Wormhole types, system effects, J-space statics, the
  signature catalogue, and Jove observatories — data the SDE lacks — are committed as
  JSON in `data/static/` and seeded into the database as the source of truth.

## Open questions (rollup)

Decisions still to settle, grouped by domain. Each is also flagged as an **Open**
blockquote next to the relevant table.

**[Mapping](./mapping.md)**

1. **Statuses** — final `map_solar_system_details.status` enum; free-text vs.
   reference for `occupying_group`.
2. **Connection vocabulary** — confirm `mass_status` / `time_status` / `size` wording
   and thresholds (esp. "super EOL < 1h" → `critical`).
3. **Reconcile-on-link** — when two scanned signatures with differing state are linked
   into a connection, which side's state wins before the trigger equalises them?
4. **Numeric vs. qualitative** — is `mass_status` / `time_status` enough, or do we
   also want numeric remaining mass / remaining lifetime on wormhole signatures?

**[Authentication](./authentication.md)**

5. **SSO flow** — Authorization Code with a server-side secret (assumed), or also
   support PKCE? Decides whether `oauth_login_flows.code_verifier` is used.
6. **Token storage** — persist the short-lived `access_token` in `tokens` (modelled),
   or keep it in cache/memory and store only the refresh token? Plus the encryption
   mechanism for `refresh_token` at rest.
7. **Token duplicates** — one token per distinct scope set per character (re-auth
   replaces), or one row per authorization?
8. **Scope catalogue seeding** — full ESI list, only requested scopes, or
   insert-on-first-seen.
9. **State store** — ephemeral session state (`oauth_login_flows`, and the
   per-session active character) in Postgres vs. Redis/cache; whether to add a
   `sessions` table.

**[Access](./access.md)**

10. **Owner constraints** — restrict `owner` / `manager` to `character` subjects? One
    owner per map, or co-owners allowed?

**[Tracking](./tracking.md)**

11. **Status history** — keep only the current snapshot or a movement history (trails /
    replay)?

**[Universe](./universe.md) / [Item types](./types.md)**

12. **Sovereignty cadence** — refresh interval for `system_sovereignty` (see
    [sovereignty refresh](../processes.md#sovereignty-refresh)).
13. **SDE reload** — how/when the SDE reference tables are (re)loaded into Postgres on
    an SDE update, and whether names are stored as English text or `jsonb` of locales.

**[Custom static reference](./static.md)**

14. **Static seeding** — how `data/static/` JSON loads into Postgres (a seeder /
    migration vs. runtime), and reconciliation when a file changes. The multi-value
    fields (`src`, `statics`, `spawn_areas`, effect modifiers) are modelled as join
    tables — confirm that over array columns.
