# Legacy analysis: the signatures panel

How the Signatures panel behaves in the legacy WormholeSystems project
(`~/Documents/Code/PHP/wormholesystems`, Laravel + Inertia + Vue 3). Written as a
reference for rebuilding the panel in Vector. File paths are relative to the legacy repo.

## 1. Component architecture

```
pages/maps/ShowMap.vue                          grid slot "signatures"
└─ components/signatures/Signatures.vue         panel chrome, header, filters, row loop
   ├─ SignaturesEmptyState.vue                  "Select a system" (when nothing selected)
   ├─ Signature.vue                             one row
   │  ├─ SignatureTypeInput.vue                 type combobox for non-wormhole rows
   │  ├─ WormholeTypeInput.vue                  type combobox for wormhole rows
   │  ├─ MapConnectionInput.vue                 connection select (wormhole rows only)
   │  └─ SignatureTimeDetails.vue               age cell + tooltip + lifetime/mass colors
   └─ PasteSignatureWarningDialog.vue           system-mismatch confirm
Composables: usePasteSignatures, useSignatures, useSortedSignatures, usePermission.
Parser: lib/SignatureParser.ts. Catalog: data/signatures.json + const/signatures.ts.
```

The panel uses the shared `MapPanel` shell (see the panels in `ui/map-panel/`). It is
shown to every role, viewers included (unlike Notes).

## 2. Panel chrome

Header title `Signatures`, then two count spans in the `<h3>`:

- `<span class="ml-1 text-amber-400">{filtered count}</span>` when > 0 (post-filter count)
- `<span class="ml-1 text-muted-foreground/70">{N} hidden</span>` when filters hide rows

Header actions, in order:

1. Compact toggle (all roles): icon button `Rows2`/`Rows3` `size-3.5`, tooltip "Switch to
   comfortable/compact signature list". Persists `compact_signature_list` on the per-map
   user settings record.
2. Category filter `ToggleGroup type="multiple" size="sm" variant="outline"`, items
   `size-6` showing just the category icon `size-3` in the category color, tooltip = full
   label. All roles. The *hidden* set persists in localStorage key
   `signatures-category-hidden-filters` (so new categories default to visible).
3. Member+ only:
   - `Unselect` text button, only while a paste selection is held.
   - Destructive trash icon button, only while the paste marks signatures as missing:
     tooltip "Delete missing signatures and their connections".
   - Paste icon button (`ClipboardPaste`), tooltip "Paste signatures from clipboard
     (Ctrl/Cmd + V)".
   - Plus icon button, tooltip "Create new signature". Creates an empty signature
     (`signature_id: ''`, no category/type).

Action buttons are `h-6` ghost buttons (`w-6 p-0` for icon size).

### Column header row

Always rendered (even with zero rows):

```
flex items-center gap-2 border-b border-border/30 bg-muted/20 px-3
font-mono text-[10px] tracking-wider text-muted-foreground uppercase
py-0.5 (compact) / py-1.5 (comfortable)
```

| Label | Width | Sortable |
|---|---|---|
| ID | `w-16 shrink-0` | yes |
| Cat | `w-24 shrink-0` | yes |
| Type | `min-w-0 flex-1` | yes |
| Conn | `min-w-0 flex-1` | no |
| Age | `w-10 shrink-0 justify-end` | yes |
| (actions spacer) | `w-14 shrink-0` | no |

Sort buttons get `ArrowUp`/`ArrowDown size-3` when active. Sort preference persists in a
cookie (`sort_preferences`, per panel key `signatures`), default `id desc`. Age sorting
compares the "modified date" (see below) newest-first in `asc`; ties fall back to
signature id ascending. Null values sort last.

### Empty bodies

- No system selected: the whole panel is replaced by an empty-state panel, centered
  `font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase` "Select a
  system".
- System selected, no rows: same centered style, text `No signatures`, or
  `{N} hidden by filters` when filters hide everything.

## 3. The row

Root:

```
flex items-center gap-2 border-b border-border/30 px-3 hover:bg-muted/30
data-deleted:bg-red-500/10 data-new:bg-green-500/10 data-updated:bg-amber-500/15
py-0.5 (compact) / py-1.5 (comfortable)
```

`data-new` / `data-updated` / `data-deleted` are set from the paste diff (section 5). No
animations, flat background tints only. Deleted-tinted rows stay fully interactive.

### ID cell (`w-16 shrink-0`)

Display: a `font-mono text-xs hover:text-amber-400` button showing `signature_id` or
`---`. Click (member+) swaps to an inline input: `maxlength=7`, placeholder `XXX-XXX`,
uppercase mono. Input strips non-alphanumerics, uppercases, auto-inserts the dash after
3 chars. Enter/blur saves (null when emptied), Escape cancels.

### Category cell (`w-24 shrink-0`)

A `Select`, disabled for viewers. Trigger shows category icon (colored) + abbreviation;
placeholder `Category` when uncategorized. Options list all 8 categories with icon +
full name. **Changing category clears both the type and the connection link**
(`signature_type_id: null, map_connection_id: null`).

Category vocabulary (id, name, abbrev, lucide icon, color):

| id | name | abbrev | icon | color |
|---|---|---|---|---|
| 1 | Wormhole | WH | Fan | text-sky-400 |
| 2 | Data Site | Data | Database | text-cyan-400 |
| 3 | Relic Site | Relic | Landmark | text-amber-400 |
| 4 | Combat Site | Combat | Swords | text-green-400 |
| 5 | Gas Site | Gas | Cloud | text-orange-400 |
| 6 | Ore Site | Ore | Gem | text-yellow-400 |
| 7 | Homefront Operations | HF | Shield | text-rose-400 |
| 8 | Factional Warfare Site | FW | Flag | text-fuchsia-400 |
| – | (uncategorized filter) | – | CircleHelp | text-muted-foreground |

### Type cell (`min-w-0 flex-1`, plus `max-w-44` on wormhole rows)

Options = catalog types of the row's category, filtered to those whose `spawn_areas`
include the selected system's class, sorted by destination-class weight. The catalog
(`data/signatures.json`, 271 types) encodes per type: category, `signature` code (e.g.
`A009`, wormholes only), `target_class` (wormholes only), `spawn_areas`. Homefront and
FW have no types at all.

- Non-wormhole rows use a combobox with a pinned "Unknown" row; it also displays
  `raw_type_name` (free text from a paste that didn't match the catalog) when no type is
  chosen. Disabled when no category.
- Wormhole rows use a sectioned combobox: **Statics** (only when the per-map user setting
  `show_statics_first` is on; the system's static codes), **K162**, **Wormholes**. Once a
  connection is linked, every section is narrowed to types whose `target_class` equals
  the linked system's class.

Wormhole types are never inferred from a paste; the user picks them.

### Connection cell (`min-w-0 flex-1`, wormhole rows only)

Non-wormhole rows omit this div entirely, so their type cell absorbs the column
(the row misaligns with the header; legacy quirk).

A `Select` with:

- `Unknown` item (= unlink, sets `map_connection_id: null`)
- group **Connections**: connections of the selected system not yet claimed by a
  signature
- group **Connected**: connections already claimed by another signature

If the signature has a typed `target_class`, both groups filter to connections whose
other endpoint has that class. Option/selected rendering: colored class short label +
target alias (medium) + system name + region (dimmed). Placeholder text `Connection`.
Sort: aliased systems first (alias compare), then by system name.

### Age cell (`w-10 shrink-0 text-right`)

`font-mono text-xs tabular-nums text-muted-foreground`, content `Nd` / `Nh` / `Nm` /
`now`, ticking every second against UTC now.

Time base: wormhole rows use `created_at` (and, when linked,
`min(signature.created_at, connection.created_at)`); other rows use `updated_at`.

Tooltip grid: `Created at` / `Last modified at` as `MMM dd, HH:mm`, plus
`End of Life (<4h)` or `Critical (<1h)` with a strict distance ("about 2 hours ago")
when a lifetime state is set.

Color coding via data attributes (connection state bleeds in: connection mass wins;
connection lifetime applies when the signature's own lifetime is `healthy`):

- lifetime `eol` purple-500, `critical` red-500
- mass `reduced` orange-500, `critical` red-500, `unknown` neutral-500
- combined lifetime+mass states pulse (2s infinite) between the two colors

### Actions cell (`w-14 shrink-0 justify-end gap-1`)

- Copy-bookmark button (wormhole rows, **all roles**): `Copy size-3.5` ghost `size-6`.
  Copies a formatted bookmark name and toasts.
- Overflow `MoreVertical` menu (member+ only), `w-44`, items `text-xs`:
  - wormhole rows: Mass radio group (Fresh neutral-500 / Reduced amber-500 / Critical
    red-500 dots), separator, Lifetime radio group (Healthy neutral / End of Life
    purple-500 / Critical red-500), separator.
  - linked rows additionally: `Preserve mass` toggle item (Heart icon, trailing Check
    when on; writes `preserve_mass` on the connection), separator.
  - always: `Delete Signature` (Trash icon, destructive).

Setting lifetime also stamps `lifetime_updated_at` client-side. Setting mass to
"unknown" writes null.

## 4. Sizing recap

- Compact vs comfortable: row padding `py-0.5` vs `py-1.5`; inner controls `h-5` vs
  `h-6`; select triggers get `!h-5 !py-0` in compact.
- Text: header/empty `font-mono text-[10px] uppercase tracking-wider`; cells `text-xs`,
  mono for ID and Age.

## 5. The paste system

### Entry points

- Window-level `paste` event: suppressed while an input/textarea is focused or no system
  is selected. No textarea in the panel, no modifier-key variants.
- The header paste button reads `navigator.clipboard.readText()` (error toast when
  unavailable).

### Parser (`lib/SignatureParser.ts`)

Lines split on `\n`, columns on `\t`. Needs >= 4 columns:
`id \t scan-group (ignored) \t category \t type-name`; trailing columns (signal %,
distance) ignored. Empty id = error toast, row dropped. Category matches by exact name,
falling back to the first ` - `-separated segment (handles
"Factional Warfare Site - Combat Site"). Type matches by exact name within the category,
**except wormholes, which never match a type from paste**. When a category matched but
the type didn't, the raw string is kept as `raw_type_name`. No locale handling.

### Diff and highlight

The paste is held client-side as `pasted_signatures` and diffed against the system's
rows by `signature_id`:

- in paste, not in DB → `new` (green tint after the server round-trip creates it)
- in paste and in DB → `updated` (amber tint)
- in DB, not in paste → `deleted` (red tint), **not deleted automatically**

The selection clears when: the active system changes, any signature event arrives on the
map channel, the user clicks `Unselect`, or missing rows are deleted.

### Lazy delete

No confirm dialog. While `deleted` rows exist, the header shows the destructive trash
button; clicking it bulk-deletes those signatures **with**
`remove_map_solarsystems: true`, which cascades: a linked signature's connection is
deleted (unless another same-side signature still references it), and connection
endpoints that are unpinned and left with no connections are removed from the map.

### System-mismatch warning

If the user's tracked character is in a different system than the selected one, the
paste is held and a dialog asks "System Mismatch Warning … Paste Anyway / Cancel".

### Server semantics (paste upsert)

One transaction, one undoable event group. New rows are created with category/type/raw
name. Existing rows (matched by `signature_id`) update exactly:

- `signature_category_id`: pasted value, else keep
- `signature_type_id`: kept when the paste has no better information; **an existing
  wormhole type always survives a repaste**; a non-wormhole category with an unmatched
  raw name clears the type
- `map_connection_id`: kept while the pasted category is null or Wormhole; **cleared when
  the paste recategorizes the row to a site**
- `raw_type_name`: cleared for wormholes, else pasted value or keep
- ship-size sync on the linked connection

`created_at` is never touched on update, so wormhole ages survive repastes. Signature
ids must be exactly 7 chars server-side.

## 6. Interplay with connections

- **Link**: pick a connection in the row's select. Server-side sync then applies
  worst-wins merging: lifetime severity `healthy < eol < critical`, mass severity
  `unknown < fresh < reduced < critical`; the merged value is written to both the
  signature and the connection, `lifetime_updated_at` bumped only on actual change. The
  connection's `ship_size` is locked from the signature's wormhole type.
- **Edit from either side**: editing the connection runs the symmetric merge across the
  connection and all linked signatures. (In Vector this lives in the DB sync triggers of
  migration 0009.)
- **Unlink**: plain `map_connection_id: null`; the connection stays on the map.
- **Delete signature**: also deletes the linked connection unless another signature on
  the same side references it; with the bulk "missing" path, orphaned unpinned endpoint
  systems are removed too.
- **K162 vs outbound**: the wormhole code lives only on the signature. The connection
  shows no code; it inherits `ship_size` and appears in bookmarks. Linking narrows the
  signature's type choices to the linked target's class.

## 7. Permissions

| Control | Viewer | Member+ |
|---|---|---|
| Compact toggle, category filters | working | working |
| Unselect / delete-missing / paste / new buttons | hidden | shown |
| ID edit, category/type/connection selects | read-only | editable |
| Copy bookmark | working | working |
| Row overflow menu | hidden | shown |

The panel itself is visible to viewers (Notes is not).

## 8. Housekeeping

- Expiry job: wormhole signatures older than 3 days and all signatures older than 7 days
  are purged server-side.
- Sorting is client-side only; the server sends rows unordered.
- Toasts on every mutation (create/update/delete/paste/bulk delete).

## 9. Differences from Vector today

Ported in full as of the signatures-panel rebuild: column table with sorting, compact
mode, category vocabulary/icons/filters, catalog-backed category+type selects (served by
`/api/signature-types`), inline ID editing, the clipboard paste flow (window paste +
clipboard button, no textarea), the paste diff with new/updated/missing tints and lazy
delete (with the legacy connection + orphan-endpoint cascade), the system-mismatch
dialog, age cells with lifetime/mass color coding and pulse states, the
mass/lifetime/preserve-mass row menu, copy bookmark, and viewer read-only gating.

Deliberate divergences:

- Sort preference persists in localStorage, not a cookie.
- No Factional Warfare category (absent from Vector's catalog data; combined FW labels
  fall back to their site segment via the parser's segment rule).
- The paste selection does not clear on other users' signature events (Vector's WS layer
  refetches without parsing frames); it clears on system change, Unselect, and delete.
- A wormhole row's raw scanner name is preserved rather than nulled on repaste.
- `preserve_mass` is stored and toggleable but unused until jump-mass tracking exists.
- No toasts; mutations report through the map status line.
