# Legacy analysis: the solar system node

What a placed solar system looks like and how it behaves in the legacy WormholeSystems
project (`~/Documents/Code/PHP/wormholesystems`, Laravel + Inertia + Vue 3). Written as a
reference for rebuilding the map in Vector. File paths below are relative to the legacy
repo; line numbers are from its current working tree.

## 1. Component architecture

The map canvas is a self-contained subsystem under `resources/js/map/` with `map/api.ts`
as its only public surface. One node renders through this chain:

```
pages/maps/ShowMap.vue                      page, panel grid, <MapRoot>
└─ map/components/MapRoot.vue               store owner, gesture list, overlays, keyboard
   └─ map/components/MapViewport.vue        scroll surface, pointer-gesture arbiter, one
   │                                        canvas-wide ContextMenu
   └─ map/components/nodes/MapNode.vue      store-connected wrapper, one per system
      ├─ NodeCard.vue                       presentational card, wrapped in an Inertia <Link>
      ├─ SolarsystemDragHandle.vue          only if canWrite && !pinned && !layoutLocked
      └─ SolarsystemConnectionHandle.vue    only if canWrite
```

State is a provide/inject store (not Pinia) in `map/store/`: `entities.ts` (systems,
connections, positions, node sizes in shallow-reactive Maps, plus a position lock set),
`viewState.ts` (scale, selection, hover, marquee, active gesture), `derived.ts` (layouts,
render positions), and `sync/` (websocket upserts). Positions live in their own Map so
entity readers do not re-render during drags.

Gestures are arbitrated by a single-owner system (`map/interactions/gestures.ts`):
gestures claim a pointer in registration order, with per-gesture hysteresis (4 px unless
noted) and pointer capture on commit. Registration order: **linkDrag, nodeDrag, pan,
marquee** (MapRoot.vue:95).

DOM hooks used by the gestures: `data-node-id` on the wrapper, `data-drag-handle`,
`data-connect-handle` + `data-connection-source`, and `data-solarsystem-id` on the card.

## 2. What the node displays

The card is a grid, `h-[40px]` (or 60 px with a pilots row), `rounded` 4 px, 1 px border,
white / dark neutral-900 background, `text-xs`. Width is intrinsic unless the tree layout
or the map's "constant width" setting forces `w-[180px]` (then the name truncates).

Row one, left to right:

| Item | Source | Presentation |
|---|---|---|
| Class / security band | static class data joined client-side | Short colored label: `C1`..`C18`, `H`, `L`, `N`, `P`, `?`. Class color tokens: c1 cyan-300, c2 blue-500, c3 purple-300, c4 violet-500, c5 orange-400, c6 red-500, c12 teal-300, c13 fuchsia-400, c14-18 rose-400, hs green-500, ls orange-500, ns red-500, pochven red-700. No numeric security is shown. |
| Alias | `alias` | Plain text before the name; when present, the real name dims to muted. |
| System name | static data | Text; truncates in fixed-width mode. |
| Occupier | `occupier_alias` | Appended in parentheses, muted. |
| Icon cluster | see below | Right-aligned 14 px icons, each with a tooltip. |

The icon cluster (each `size-[14px]`, tooltips at 500 ms delay):

| Icon | Condition | Meaning |
|---|---|---|
| Status glyph | status set and not `unknown` | friendly ShieldCheck, hostile Skull, active Activity, unscanned Radar, empty CircleDashed, unknown CircleHelp |
| Home (lucide Home, amber-400) | map's home system | "Home system" |
| Flag (red-400) | map's rally point | "Rally point" |
| Lock (muted) | `pinned` | "Pinned in place"; also hides the drag handle |
| Satellite | signature counts | rose-500 if any uncategorized signatures, else amber-500; the count itself is tooltip-only ("N signatures, M uncategorized") |
| Fan (sky-500) | `wormhole_signatures_count > map_connections_count` | "Has N unmapped wormholes" |
| Aperture (amber-500/90) | shattered system | "Shattered system" |
| Sovereignty logo (16 px) | alliance, else corp, else faction | Tooltip: logo + name + ticker. Fetched lazily from `/api/sovereignties` when not preloaded. |
| Effect badge | only as **fallback when no sovereignty** | 14 px circle with a letter: Pulsar P blue, Magnetar M pink, Wolf-Rayet W amber-950, Black Hole B gray, Red Giant R red, Cataclysmic Variable C yellow. Click opens a popover listing every modifier with green up / red down arrows. |

Row two of the inner grid, mutually exclusive:

- K-space: region name, muted.
- W-space: the statics, one colored span each. The label is `leads_to` with the trailing
  `s` stripped and uppercased (`c5s` becomes `C5`, `hs` becomes `H`), colored by
  destination class. Each static has a rich tooltip (700 ms): wormhole code and target,
  total mass, max jump mass, ship size class (XL/L/M/S by jump mass), lifetime, signature
  strength.

Optional third row (card grows to 60 px): online pilots. A pulsing green dot, the first
pilot's name, and "and N more". The full pilot list lives in a scrollable tooltip:
portrait, name, corp ticker, current ship type. Pilot presence comes from
`map_characters` filtered to this system; the server restricts it to online, tracked,
map-authorized characters.

Not on the node: kill/activity indicators (killmails are a sidebar panel), wormhole
life/mass state (rendered as badges on the connection edges), signature numerals, or
numeric security.

### Styling channels

Four independent visual channels on the card:

- **Border color encodes status**: active orange-500, empty emerald-400, friendly
  blue-500, hostile red-500, unscanned cyan-500 (dark mode only; light mode falls back to
  neutral), unknown/default neutral-300 / neutral-700.
- **Selected** (marquee selection): amber-100 / amber-900 background.
- **Active** (the system whose detail panels are open): `ring-2 ring-amber-500` with ring
  offset, plus z-10.
- **Hovered**: `outline-2 outline-yellow-500`; hover also raises z-20 and reveals the two
  handles. Sidebar rows (characters, killmails, skyhooks, EVE Scout) can set the hover
  highlight from outside the canvas.
- **Threat ring** (optional, user setting): `ring-2` in red-600 (critical) or orange-500
  (high), suppressed while the node is active.

### Geometry

The stored position is the **connection anchor**, not the top-left: the card's top-left
is `anchor - {x: 40, y: 20}`, node height 40, applied as a CSS translate pre-multiplied
by the zoom scale. Zoom is a CSS `scale` on the card (0.5 to 2.0 in 0.1 steps, persisted
in a cookie); node sizes are measured by a shared ResizeObserver into the store for edge
routing, in unscaled base units. Canvas default 4000 x 2000, grid size 20.

## 3. Interactions

### Left click

No click-to-select. The whole card is an Inertia `<Link>` to
`GET /maps/{slug}?solarsystem_id=...` with a partial reload of `map`,
`selected_map_solarsystem`, `map_navigation`, `map_characters`, `eve_scout_connections`,
and `threat_analysis` (prefetched on hover, cached 2 s). This makes the node the
**active** system and drives every side panel: system info, notes, signatures, threat
analysis, navigation, characters, killmails, EVE Scout, skyhooks, audits. Left click
never touches the marquee selection.

### Double click

Opens the **alias editor popover** anchored on the system name: two inputs (Alias,
Occupier alias) and a Save button, submitting `PUT /map-solarsystems/{id}`. Nothing else
is bound to double click. Side effect: the underlying link navigation also fires.

### Right click: the context menu

One canvas-wide ContextMenu; the target node is resolved from `data-node-id` on the event
target. Right click does not change selection and does not make the node active. Entries
in order (`map/components/overlays/MapSolarsystemContextMenu.vue`):

| Entry | Shown when | Action |
|---|---|---|
| Pin / Unpin | can write | `PUT /map-solarsystems/{id}` `{pinned}` |
| Add connection | can write | opens the add-connection dialog (combobox search; picking a system creates it and connects it) |
| Status (submenu) | can write | radio list unknown / friendly / hostile / active / unscanned / empty, each with its status icon; `PUT {status}` |
| External (submenu) | always | Dotlan system / region map / jump range (k-space only), zKillboard system / constellation / region, all new-tab |
| Set destination (submenu) | logged in, k-space only | one item per online character, plus "All characters"; `POST /waypoints` with `clear_other_waypoints: true` |
| Add waypoint (submenu) | same | same list with `clear_other_waypoints: false` |
| Route planner (submenu) | always | set as origin / set as destination (client-side state for the shortest-path panel) |
| Set / Unset home system | can write | `POST /maps/{map}/settings/home-system` |
| Set / Clear rally point | can write | `POST /maps/{map}/settings/rally-point` |
| Remove (destructive) | can write, not pinned, not home | **no confirmation**; if a marquee selection exists this deletes the whole selection (`DELETE /map-selection`), otherwise `DELETE /map-solarsystems/{id}` |

Confirmation dialogs exist only on the map background menu (Clear map / Clean map).

### Dragging

- Only via the **drag handle**: a 12 x 2 px pill centered on the top edge, visible on
  hover, and only rendered for writers on unpinned systems outside the locked tree
  layout. Dragging the card body does nothing (it is the link).
- 4 px hysteresis, then pointer capture and `user-select: none`.
- **Group drag**: if the grabbed node is in the marquee selection, the whole selection
  minus pinned and home systems moves together from snapshotted start positions.
  Dragging an unselected node clears the selection on first move.
- Movement is optimistic: dragged ids are position-locked so websocket echoes cannot
  snap them back; every frame applies grid snapping (20 px) and clamping to the canvas.
- Drop: group commits via `PUT /map-selection` with all positions (and clears the
  selection on success); a single node commits `PUT /map-solarsystems/{id}` with
  `suppress_notification: true`. Errors trigger a map reload; pointercancel restores the
  snapshots.

### Making connections

- The **connection handle** (16 px circle at the right edge, hover-only, writers only)
  claims the pointer with zero hysteresis and wins over all other gestures. A ghost curve
  follows the cursor.
- Drop resolves the target with `document.elementFromPoint` to the nearest
  `data-node-id`; self-drop or empty space aborts silently.
- `POST /map-connections` with a heuristic default ship size: C1 or Turnur or
  Thera-to-highsec involved gives medium, C13 gives frigate, otherwise large. No
  optimistic edge insert.

### Selection

- Only the **marquee** selects: plain left drag on empty canvas (in the locked tree
  layout it additionally requires Shift or Ctrl). Selection commits live during the drag
  (anchor-point-in-box hit test), stays after release, and a plain background tap clears
  it. There is no shift-click or ctrl-click on nodes at all.
- A selection enables: group drag, Delete-key bulk delete, the background menu's "Delete
  selection" and "Organize selection" (stack into a column with configurable spacing,
  committed via `PUT /map-selection`), and it hijacks the node menu's Remove into a bulk
  delete.

### Other input

- **Middle click**: always pans (grabbing cursor); a middle tap does nothing and native
  middle-click-open-link is not suppressed.
- **Keyboard**: Delete deletes the selection (no confirmation, suppressed while typing in
  inputs); Cmd/Ctrl+K opens a command palette whose system results navigate to the
  system and scroll its node into view, or add the system to the map if absent. No arrow
  nudging, no Escape-clears, no select-all.
- **Hover**: sets the store's hovered id (yellow outline), reveals the handles, and is
  also settable from sidebar rows to highlight a node from outside the canvas.
- **Touch**: nothing touch-specific exists; handles are hover-revealed, so touch cannot
  reach them, and there is no pinch zoom or long-press menu.

## 4. Endpoint map for node interactions

| Interaction | Request |
|---|---|
| Left click | `GET /maps/{map}?solarsystem_id=` (partial Inertia reload) |
| Alias save (double click) | `PUT /map-solarsystems/{id}` `{alias, occupier_alias}` |
| Single-node drop | `PUT /map-solarsystems/{id}` `{position_x, position_y, suppress_notification}` |
| Group drop / organize | `PUT /map-selection` `{map_solarsystems: [{id, position_x, position_y}]}` |
| Delete selection | `DELETE /map-selection` `{map_solarsystem_ids}` |
| Remove single | `DELETE /map-solarsystems/{id}` |
| Pin / status | `PUT /map-solarsystems/{id}` `{pinned}` / `{status}` |
| Home / rally | `POST /maps/{map}/settings/home-system` / `rally-point` |
| Waypoints | `POST /waypoints`, `POST /waypoints/bulk` |
| Connection drop | `POST /map-connections` `{from, to, ship_size}` |
| Add-connection dialog | `POST /map-solarsystems` with `connect_to_map_solarsystem_id` |

## 5. Differences from Vector's current node

As of the parity port, the node and its interactions match legacy in: status vocabulary
and colors, class color tokens, icon cluster (status/home/rally/pin/signatures/unmapped/
shattered/sovereignty/effect), statics with physics tooltips, pilot presence row, alias
editor on double click, active-system click model with side panels (System Info,
Signatures, Notes, Navigation, Threat), marquee-only live selection with hysteresis,
context menu structure (status and external submenus, waypoints, route planner, remove
gating and selection hijack), ship-size heuristic on connection create, threat rings, and
client-side routing with edge highlighting.

Still intentionally different or deferred: intrinsic node width (Vector stays fixed
180px), the anchor-point position model (Vector stores top-left), command palette, tree
layout, organize-selection, watchlist and Find tabs of navigation, EVE Scout integration,
killmails/audits/ship-history panels, presence realtime push (Vector polls), and the
Dotlan region-map underscore handling for named regions is shared.
