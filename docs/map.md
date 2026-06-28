# Interactive Map — Design Document

> Status: **Draft.** Dictated by the user, transcribed and grounded against the codebase.
> Items needing a final call are collected in **§9 Open decisions** (each with a recommended
> default). Sections marked _(required)_ are hard constraints.

## 1. Overview

A modern, interactive map that displays a wormhole network: solar systems as draggable nodes
connected by edges, on a pannable/zoomable canvas, with real-time multiplayer updates. Builds
on the existing map stack:

- `MapView` (`src/maps/mod.rs`) — `{ map, systems: Vec<MapSolarSystem>, connections }`
- `MapSolarSystem` (`src/maps/solar_system.rs`) — `id, map_id, solar_system_id, position_x,
  position_y, alias, created_at`
- Server fns in `src/app/api.rs` (`FetchMapFn`, `MoveSystemFn`, `AddSystemFn`,
  `AddConnectionFn`, `SearchSystemsFn`, …)
- `MapHub` + per-map WebSocket (`/ws/map/{map_id}`) for real-time — already wired (§8)

## 2. Visual design

Overall feel: **modern**.

### 2.1 Grid background

- The canvas has a **grid background**.
- Grid cell size is **configurable via the config** — the single source of truth the rest of
  the layout derives from (see §9.7 for which config).

### 2.2 System node

One placed solar system on the map.

- **Height:** exactly **twice the grid cell height**, so nodes align to the grid.
- **Width:** **fixed**.
- **Text:** **truncated within the node** (no overflow outside the node bounds).

#### Node content

- **Name line:** `[alias] <system name> [occupier]` — the **alias** renders **before** the
  original system name, the **occupier** **after** it.
  - **System name** — from `solar_systems` (joined on `solar_system_id`).
  - **Alias** — `map_solar_systems.alias`; **manually** entered by the user.
  - **Occupier** — `map_solar_system_details.occupying_group`; **manually** entered, same as the
    alias (free text the user types in). _Note: no Rust code touches
    `map_solar_system_details` yet — struct, reads, and writes are all net-new._
- **Sovereignty** — the holder, joined from `system_sovereignty` (§6.2 / §7). Modeled as a
  data-carrying enum `Sovereignty` (`Alliance`/`Corporation`/`Faction`), each variant carrying
  the holder's **id** + **name** (+ **ticker** for alliance/corp). The variant + id let the node
  render the holder's **icon** from the right EVE image endpoint. Distinct from the manual
  occupier above.
- **Security / system class** — `solar_systems.security_status` (the column is
  `security_status`, not `security`; `SystemSearchResult` aliases it `as "security!"`) plus
  `wormhole_class_id`.
- **Statics _or_ region:**
  - **Wormhole systems:** the system's **statics** and **what class each leads to** (e.g.
    → high-sec, → C4). Source: `wormhole_system_statics` → `wormhole_types.dest_class`.
  - **Non-wormhole systems:** the **region** (`solar_systems.region_id` → `regions.name`).
- **Wormhole effect indicator:** if present, a **small icon** for the effect (Pulsar, Black
  Hole, Magnetar, …). Source: `wormhole_systems.effect_name`.
  - **Hover or click → popover** explaining the effect's **buffs/debuffs**. Source:
    `wormhole_effect_modifiers` (`kind`, `stat`, `value`), filtered to the system's
    `wormhole_class_id` (magnitudes vary by class).

> A shared `wormhole_class_id → label` formatter serves both the security/class line and the
> statics' destination labels.

### 2.3 Canvas, viewport & panning

- **Fixed world size:** a fixed **4000 × 2000** world. System `position_x/y` live in this space.
- **Viewport:** **full width × 1400 px height** (for now) — a window onto the larger world.
- **Not natively scrollable** — no browser overflow scrolling.
- **Pan:** hold the **middle mouse button** and drag.
- **Virtual scrollbars:** **custom** scrollbars we render, representing the viewport's position
  in the world and draggable to move around (not native overflow).
- Pan/scroll compose with zoom (§3.3): the visible world region is a function of pan offset and
  zoom factor.

### 2.4 Preventing default browser behavior _(required)_

The map hijacks native gestures, so `preventDefault()` / `stopPropagation()` (and CSS) are used
generously:

- **Middle-mouse pan** — suppress the browser's middle-click **autoscroll**.
- **Zoom** is via dedicated buttons (§3.3), **not** the wheel. If we trap `wheel` at all (e.g. to
  stop the page scrolling over the canvas), the listener must be **non-passive**.
- **Right-click menus** (§5) — `preventDefault` the native `contextmenu`, show ours.
- **Drag / rubber-band** (§3.1, §3.4) — prevent native text/image selection and `dragstart`
  (`user-select: none`).
- **Owned keyboard shortcuts** (e.g. Delete — §3.5) when the map has focus.

## 3. Interactions

### 3.1 Drag handle

- Each node has a small **drag handle** on **top**, **visible only on hover**; hidden otherwise.

### 3.2 Dragging & persistence

- Drag a system (via the handle); on **drop**, the new position **auto-saves** (`MoveSystemFn`).
- **Pinned systems can't be moved** — pinning a system **drag-locks** it (the drag handle is
  disabled/hidden for pinned nodes). Pin/unpin via the node menu (§5.2). This is the primary
  meaning of "pinned"; it also makes the system survive Clear map (§5.1).

### 3.3 Zoom

- Zoom is controlled by **dedicated zoom-in / zoom-out buttons** (an on-canvas control), **not**
  the mouse wheel.
- Dragging **and** persistence **account for the current zoom**: screen-space pointer deltas are
  converted to world space by the active zoom factor before being applied and saved. **Stored
  coordinates are world-space, independent of zoom.** (Same applies to rubber-band selection,
  §3.4.)

### 3.4 Rubber-band selection

- Hold the **left mouse button on empty map** and drag to draw a **selection rectangle**.
- The preview **updates in real time**: the rectangle draws and intersecting nodes show a
  selection highlight, so it's clear what's selected / about to be.
- Intersection is tested in **world space** (convert the screen-space rectangle by pan + zoom),
  so it's correct at any zoom.

### 3.5 Multi-select operations

- Multiple systems can be selected (via §3.4).
- **Delete key**, or **right-click a selected system → Remove solar systems**, deletes **all**
  selected systems.
- Goes through a **bulk server fn** (`RemoveSystemsFn`, takes a list of `map_solar_system_id`s)
  — not N calls — emitting a **single coarse refetch event** (§8.2). "Clear map" (§5.1) shares
  this path.

## 4. Connections

### 4.1 Creating a connection

- Each node has a **connection handle** on its **right edge** (distinct from the top drag
  handle).
- **Dragging from it** starts creating a connection; releasing over a target system creates the
  edge (`AddConnectionFn` → `map_connections`).
- While dragging, render a **live preview** — a **smooth curve** following the pointer.

### 4.2 Rendering

- All connections render in a **dedicated SVG** layer.
- **Smooth bézier curves**, not straight lines.
- **Edge-anchored, not center-anchored:** connect from node **edges** (nodes are fixed-size, so
  this looks cleaner). The preview uses the same anchoring.
- **Hover effects** on connections.

## 5. Context menus

Right-click menus; options grounded in the existing schema/enums.

### 5.1 Map context menu

- **Add solar system** — opens the solar-system search dialog (existing search palette /
  `SearchSystemsFn`); confirm places it (`AddSystemFn`).
- **Clear map** — removes all placed systems **except** home and pinned (delete every
  `map_solar_systems` row for this map that is neither home nor pinned). Connections cascade
  (FK `on delete cascade`). Uses the bulk path (§3.5 / §8.2). Requires §6.1.

### 5.2 System node context menu

Acts on `map_solar_systems` + `map_solar_system_details`:

- **Add connection** — opens the search dialog. On confirm: if the chosen system isn't on the
  map, add it (`AddSystemFn`); then connect **this** system to it (`AddConnectionFn`). Menu-driven
  complement to the drag flow (§4.1).
- **Rename alias** — `map_solar_systems.alias` (manual; shown before the name — §2.2).
- **Set occupier** — `map_solar_system_details.occupying_group` (manual; shown after the name).
- **Set status** — `map_solar_system_details.status`; one of the `SystemStatus` enum (§6.4).
- **Set as home** — `map_solar_systems.is_home` (clears any previous home; one per map — §6.1).
- **Pin / Unpin** — `map_solar_systems.is_pinned`. Pinning **drag-locks** the system (§3.2) and
  makes it survive Clear map. Many pinned allowed; a system may be both home and pinned.
- **Remove system** — delete the placement (`RemoveSystemFn` / cascade).

### 5.3 Connection context menu

Acts on `map_connections`; values from the enums in `src/maps/mod.rs`:

- **Type** — `ConnectionType`: `wormhole` | `stargate`. _Net-new backend: `kind`/`type` is set
  only at insert today; `set_connection_status` touches only mass/time/size. Add a type setter._
- **Mass status** — `MassStatus`: `stable` | `reduced` | `critical`.
- **Time / EOL status** — `TimeStatus`: `stable` | `eol` | `critical`.
- **Size** — `WormholeSize`: `xl` | `large` | `medium` | `small`.
- **Delete connection**.

> Setting mass/time/size propagates to a linked signature (and its sibling) via the `map_*_sync`
> triggers (migration 0009) — the menu just writes the connection.

## 6. Data-model changes

The dev DB is re-wiped/re-migrated from scratch, so these edit **existing** migrations rather
than adding ALTER migrations.

### 6.1 Home & pinned systems — edit `0005_create_maps.sql`

`map_solar_systems` has no home/pinned columns today (verified). Add:

```sql
-- on map_solar_systems:
is_home    boolean not null default false,
is_pinned  boolean not null default false,

-- at most ONE home per map; pinned is unconstrained:
create unique index map_solar_systems_one_home
    on map_solar_systems (map_id) where is_home;
```

- One home per map (partial unique index); many pinned. **A system may be both home and pinned.**
- Setting home clears any previous home for the map.
- `is_pinned` drag-locks the node (§3.2) in addition to surviving Clear map.

### 6.2 Sovereignty table — ALREADY EXISTS (no new migration)

> Corrected after cross-check: the table is **not** new. `system_sovereignty` already exists
> (`migrations/0003_create_universe.sql:143`), with FKs added in `0007`:
>
> ```sql
> solar_system_id   bigint primary key references solar_systems (id),
> alliance_id       bigint,            -- FK added in 0007
> corporation_id    bigint,            -- FK added in 0007
> faction_id        bigint references factions (id),
> claimed_since     timestamptz,
> is_capital_system boolean,
> updated_at        timestamptz not null default now()
> ```

So no migration is required for sovereignty storage. Notes vs. the original proposal:

- There is **no `claim_type` column.** Derive the claim kind in the read query from which id is
  non-null (`alliance_id` → alliance, `faction_id` → faction, else unclaimed), or add a single
  `claim_type text` column if we prefer it explicit. _Default: derive in-query, no schema change._
- The existing table has `is_capital_system` (ESI supplies it — `esi/sovereignty.rs`), which we
  can keep populating.
- Holder-entity tables (`factions`, `corporations`, `alliances`, migration 0003) already exist
  with `name` (+ `ticker` for corp/alliance).

### 6.3 Display fields on the map fetch

The node needs name/security/class/occupier/sovereignty/statics/effect. These are joined into
the map/system fetch (extend what `MapView`/`MapSolarSystem` carries) rather than per-node
lookups.

### 6.4 `SystemStatus` enum

`map_solar_system_details.status` becomes a proper Rust enum (a `text_enum!` like the connection
enums in `src/maps/mod.rs`), with variants:

- `unscanned`, `scanned`, `occupied`, `friendly`, `hostile`, `unknown`

Default stays `unscanned`. Drives the **Set status** menu item (§5.2).

### 6.5 Grid config (server-owned)

The grid cell size (and derived layout constants — §2.1) is **stored server-side** in the app
`Config` (`src/config`) and passed to / accessed by the front end when needed (rendered into the
page or read via the map fetch), so there's a single source of truth.

## 7. Sovereignty sync process

Keeps sovereignty + referenced entities current. Follows the `src/tracking.rs` pattern: a
`tokio::spawn`ed `interval` loop started from `main.rs`, with a `Semaphore` bounding ESI calls.

Per tick:

1. **Fetch** — `EsiClient::sovereignty_systems()` (`GET /sovereignty/systems`), one bulk call.
   Upsert into the sovereignty table (§6.2).
2. **Resolve entities** — each claim references `alliance_id` + `corporation_id`, or `faction_id`.
   For any **corp/alliance** id not already stored, fetch and upsert:
   - `corporations` ← `EsiClient::corporation()`
   - `alliances` ← `EsiClient::alliance()`
   - **Factions need no fetch** — they're fully seeded from the SDE (`seed/mod.rs`); there is no
     `EsiClient::faction()`.
   Known rows are skipped (refreshed weekly — §9.4). Upsert pattern already used in
   `session.rs`.
3. Nodes then join `system_sovereignty` → entity tables for display names/tickers.

> Storage already exists (`system_sovereignty`, §6.2) and so does the ESI call; only the loop is
> new. This sync is **simpler than `tracking.rs`**: `sovereignty_systems` / `corporation` /
> `alliance` are **public** ESI calls — no token, no scopes, no per-character logic, no
> `UserHub`. Only the `Semaphore`/`JoinSet` bounded-concurrency helper is worth reusing; `start()`
> needs just `pool` + `esi`.

## 8. Real-time (multiplayer)

**Goal:** any map change — system position, connection, signature, anything — broadcasts to all
**other** windows viewing that map, updating them live.

**Already implemented** end-to-end as **notify-then-refetch** (events carry *what* changed by
id; clients refetch the affected slice):

1. **Publish** — every mutating server fn calls `hub.publish(MapEvent::…)` (`src/app/api.rs`):
   `SystemAdded`, `SystemMoved`, `SystemRemoved`, `ConnectionChanged`, `SignatureChanged`, plus
   `MapUpdated` / `AccessChanged`.
2. **Fan-out** — `MapHub` (`src/maps/events.rs`): one `tokio::broadcast` channel per `map_id`.
3. **Stream** — `/ws/map/{map_id}` (`map_ws`) authenticates + role-checks, then
   `stream_map_events` forwards the map's events down the socket.
4. **Apply** — the client (`src/app/pages/map_view.rs`, `start_ws`) subscribes and bumps a
   `refetch` signal on each event; map/signature resources reload.

> The originating window also receives its own event; the client handles this idempotently
> (bumps `refetch` both locally and on the WS event).

### 8.1 Seamless updates — no visible refresh _(required)_

The live refetch must be **invisible**: the map stays on screen and updates in place.

- **No loading banner / spinner / "Loading…" fallback** on refetch.
- **No flicker, blank frame, or remount** of canvas or nodes.
- **Pan/zoom, hover, selection, and in-progress drag are preserved** across an update.

Leptos implementation notes:

- Don't gate the map on a `<Suspense>` whose fallback re-shows on refetch. Use `<Transition>`
  (keeps current view mounted while loading) or read the resource's previous value. Initial SSR
  load may show a fallback; **subsequent** refetches must not.
- Render nodes/connections **keyed by stable id** so the framework **diffs in place** instead of
  remounting — preserving per-node UI state and avoiding flicker.
- Only the changed slice visibly changes.

### 8.2 Events for the new actions

Transport is done; new mutations must publish a matching event:

- **Drag-move** (§3.2) → `SystemMoved` (exists).
- **Connection create/edit/delete** (§4, §5.3) → `ConnectionChanged` (exists).
- **Clear map / bulk delete** (§3.5, §5.1) → bulk fn emits a **single coarse event**
  (`MapUpdated` or a dedicated bulk variant) → one full refetch, no event storm.
- **Set status / occupier / home / pinned** (§5.2, §6.1) → no event exists yet. Add
  `SystemDetailsChanged { map_id, map_solar_system_id }` (§9.6).
- **Sovereignty sync** (§7) → reference-data update (§9.4 — whether to nudge open maps).

## 9. Resolved decisions

All confirmed by the user:

1. **Home + pinned overlap** — a system **may be both** home and pinned.
2. **Occupier source** — **manual** free text, just like the alias (§2.2); shown after the system
   name. Sovereignty (§7) is a separate, auto-synced field.
3. **System status** — Rust `SystemStatus` enum: `unscanned`, `scanned`, `occupied`, `friendly`,
   `hostile`, `unknown` (§6.4).
4. **Entity refresh cadence** — **once a week** for known corps/alliances (§7).
5. **Display-field plumbing** — **yes**, all node display data comes via the map/system fetch,
   not per-node calls (§6.3).
6. **`SystemDetailsChanged` event** — **yes**, add the new variant (§8.2).
7. **Grid config** — **server-owned** in app `Config`, passed to / read by the front end (§6.5).
8. **"Copper"** — was a typo, **not** an aesthetic. The look is simply "modern" (§2). No special
   palette.
9. **Rendering target** — **nodes = DOM/HTML, connections = one SVG overlay** (§4.2).

## 10. Out of scope / future

_TBD._
