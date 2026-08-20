# Legacy analysis: the map canvas

How the map *surface* behaves in the legacy WormholeSystems project
(`~/Documents/Code/PHP/wormholesystems`, Laravel + Inertia + Vue 3): zoom, the scroll
model, grid lines, background images, the custom scrollbars, and the second layout mode.
Nodes and edges themselves are covered by [solar-system-node.md](./solar-system-node.md);
this is everything they sit on. File paths are relative to the legacy repo.

## 1. Component architecture

```
pages/maps/ShowMap.vue                     page, panel grid
└─ map/components/MapRoot.vue              store owner, gestures, overlays, keyboard
   └─ map/components/MapViewport.vue       scroll surface, grid, background, marquee box
      ├─ map/components/MapScrollbar.vue   one per axis, presentational
      ├─ edges/EdgeLayer.vue               one SVG over the canvas
      └─ nodes/MapNode.vue                 one per system
map/components/MapReadonly.vue             a store-free renderer, used by the landing page
```

Canvas-relevant modules: `core/coords.ts` (the only place scale is applied),
`core/layout/treeLayout.ts` (the second layout), `core/geometry/{freeRouting,treeRouting}.ts`
(one edge router per layout), `interactions/{pan,useMapScrollbars}.ts`,
`composables/useMapBackground.ts`, `composables/useLayout.ts` (zoom persistence).

## 2. Coordinate model

Two spaces, one boundary. Everything in the map subsystem — store, geometry, layout,
hit-testing — works in **base units**; `scale` is applied only where coordinates are
emitted into the DOM, and screen input is converted straight back (`clientToBase`).

- `ANCHOR_OFFSET = { x: 40, y: 20 }`: a system's stored position is its *anchor*, not its
  top-left. Nodes render translated by minus that; edge routing reconstructs the box from
  anchor + measured size (`nodeRect`).
- `ITEM_HEIGHT = 40`: a plain card's height, two grid cells.
- `clampToCanvas` keeps an anchor no closer to the edges than the node overhang.
- `clientToBase` takes a `ViewportFrame` (rect + scroll offsets + scale) captured once at
  gesture start rather than per pointer event.

## 3. Zoom

- Range **0.5–2.0**, step **0.1**, rounded to one decimal (`MapOptions.setScale`). The
  current value is shown as a percentage between the − and + buttons.
- Persisted per user in a `layout` **cookie** (`{ map_height, scale }`) via `useLayout`,
  with a debounced (1s) server reload so other props follow. It survives reloads and
  follows the user across maps.
- `MapRoot` mirrors the layout's scale into `store.scale`; nothing else writes it.
- Zoom is **not** on the wheel. Plain wheel is left to the page, `ctrl/meta+wheel` is
  swallowed (so the browser does not zoom the app), `shift+wheel` scrolls the canvas.

## 4. The scroll model (the important structural difference)

The canvas is a **native scroll container** with `overflow: hidden`: scrolling happens by
writing `scrollLeft` / `scrollTop`, and the hidden overflow is what suppresses the native
scrollbars while keeping programmatic scrolling. Nodes are absolutely positioned at
*scaled* coordinates and each card is scaled with `origin-top-left`; the container is
sized to the scaled content via `min-width` / `min-height`.

Consequences worth keeping in mind:

- Panning is `scrollLeft/scrollTop` arithmetic (`interactions/pan.ts`), so it is bounded by
  the content size for free, with no clamping logic.
- The scrollbars, the background modes and the marquee all read from the same scroll
  offsets, so they cannot drift apart.
- Content size is computed, not measured (`MapViewport.contentSize`): the manual layout
  uses `config.max_size * scale`, the tree layout uses its own extent, both plus
  `240 * scale` padding so a node's body and handles are not clipped.

## 5. Grid lines

Two CSS gradients (`to right`, `to bottom`) at 1px, `background-size` = `grid_size * scale`
in both axes, painted on the scaled content element so the grid pans and zooms with the
systems. The colour is the `--grid` token, defined per theme
(`hsl(0 0% 90%)` light, `hsl(0 0% 12%)` dark). The grid is drawn **only in the manual
layout** (`:class="{ 'bg-grid': !isTreeLayout }"`) — an auto-placed tree has no grid to
snap to, so showing one would be a lie.

## 6. Background images

Per **user**, per map (`map_user_settings.background_image_path`, `background_image_mode`),
uploaded through `MapBackgroundImageController` and managed from the `MapOptions` popover
(click or drag-and-drop, plus a remove button). Guests cannot persist one.

Two modes, and the difference is which element the image is painted on:

| Mode | Painted on | Behaviour |
|------|------------|-----------|
| `grid` (default) | the scaled content element, layered *under* the grid gradients, `background-size: cover` as a third layer | spans the whole map, pans and zooms with the systems |
| `viewport` | the scroll container itself (which never scrolls) | fills the visible panel and stays put while you pan or zoom |

## 7. Custom scrollbars

`interactions/useMapScrollbars.ts` + `MapScrollbar.vue`. Track and thumb are plain divs
positioned over the canvas; the native ones stay hidden by `overflow: hidden`.

- **Auto-hide**: visible on scroll, on mouse-enter and on (300ms-throttled) mouse-move,
  hidden **1500ms** after the last of those. Held open while a thumb is being dragged.
- **Geometry**: `SCROLLBAR_SIZE = 8`, `MIN_THUMB_SIZE = 30`. Thumb size is
  `viewport / content * track`, floored at the minimum; offset is
  `scroll / maxScroll * (track - thumb)`. Each track shortens by the scrollbar size when
  the other axis is present, so they never overlap in the corner.
- **Interaction**: thumb drag maps pixel delta to scroll delta through the same ratio;
  a track click jumps so the clicked point becomes the viewport centre.
- **Sizing**: a `ResizeObserver` on the container *and* its first child, plus a re-measure
  on scale change (next frame). The caller passes its intended content size so a layout
  switch is reflected immediately instead of racing the DOM.

## 8. Layout modes (the second rendering mode)

The map renders in one of two modes, `manual` or `tree`.

**Where it is decided.** `maps.layout` (default `manual`) is the map's mode;
`maps.allow_layout_override` lets a viewer differ; `map_user_settings.layout_override`
holds their choice (`null` = follow the map). The effective mode is derived in
`store/derived.ts`. `MapOptions` shows both modes as toggle buttons and clears the
override when you pick the map's own mode.

**What tree mode changes:**

- *Positions.* `computeTreeLayout` (Reingold–Tilford, `core/layout/treeLayout.ts`, 346
  lines with its own spec) lays the chain out left-to-right as a spanning forest.
  Pinned systems are the roots; everything else attaches to the nearest root by BFS; the
  map's home system is the fallback root. Siblings are ordered by the shared
  `core/sorting.ts` comparator. Defaults, all snapped to whole grid cells:
  `levelGap 320`, `siblingGap 60`, `marginX 60`, `marginY 40`. Positions are derived, never
  written to the server.
- *Edge routing.* `treeRouting.ts` replaces the free router: orthogonal runs that leave the
  facing edge of each box, preferring left/right whenever the boxes are horizontally
  separated, with parallel edges fanned out along a shared node edge
  (`PARALLEL_SPACING 14`, `BEND_SPACING 16`) so they do not overlap. The manual layout uses
  `freeRouting.ts` instead: curves between "rail" endpoints that slide along each node's
  horizontal centre line, inset `RAIL_PADDING 40`.
- *Interaction.* `isLayoutLocked` = tree: node dragging and the marquee are off, and a plain
  left drag pans instead (`pan.ts`), with a `cursor-grab` on the surface.
- *Canvas.* Sized to the tree's own extent rather than `max_size`, and the grid is not
  drawn.

Related map-level option: `maps.constant_width_enabled` renders fixed-width cards, which
makes edge geometry exact on first paint with no measurement pass.

## 9. MapReadonly

A third, store-free renderer (`MapReadonly.vue`) for plain data: the landing page's demo
map. Fixed 180×40 cards, the free router's curves, the same grid background, a `scale`
prop, no interaction at all. Worth knowing it exists, because it is the cheapest way to
render a map outside the live canvas (a share view, an alert preview, a print).

## 10. WormholeSystems today, and the gaps

| Area | Legacy | WormholeSystems (`frontend/src/routes/maps/[id]/`) |
|------|--------|-------------------------------------------|
| Scroll model | native scroll container, `scrollLeft/Top` | one `translate(pan) scale(zoom)` world div |
| Zoom | 0.5–2, 0.1 steps, cookie-persisted, % readout | *ported*: same range and step, `localStorage` per map, % readout |
| Wheel | plain → page, `ctrl` → swallowed, `shift` → scroll | *ported*: same three rules |
| Grid | `--grid` token per theme, manual layout only | *ported*: `--color-grid` / `--color-canvas` per theme |
| Background image | per user, 2 modes, upload + drag-drop | **missing** |
| Scrollbars | auto-hide, min thumb, track click, drag | *ported*: auto-hide after 1.5s, min thumb, click + drag |
| Layout modes | manual + tree, per-map default, per-user override | *ported*: same three fields, switcher on the canvas |
| Tree edge routing | orthogonal with parallel fan-out | *ported*, and improved: runs are kept out of the column bands, and an edge between two nodes with something between them detours into the lane |
| Layout lock | drag/marquee off, left-drag pans | *ported* |
| Readonly renderer | `MapReadonly.vue` | **missing** |

The zoom row's persistence differs on purpose: legacy keeps one scale for every map in a
cookie, WormholeSystems keeps one per map in `localStorage`. Both are per browser rather than per
account, because how far out you want to be depends on the screen you are sitting at.

Two WormholeSystems-side details that matter for the port:

- The transform model (`translate` + `scale` on one world div) is not a scroll container,
  so panning is clamped by hand and the scrollbar thumbs are computed from `pan` rather
  than from scroll offsets. It is a legitimate choice and it already works; the cost is
  that the `viewport` background mode and content-sized canvases need explicit handling
  rather than falling out of the DOM.
- `frontend/src/lib/map/helpers.ts` already owns the shared layout maths (`freePosition`,
  `NODE_W`, `nodeAt`), and `src/maps/ghost.rs` mirrors part of it server-side. A tree
  layout would live beside it, client-side only, since its positions are derived and never
  stored.
