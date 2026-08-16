# UI style guide

The visual language for Vector's interface. Keep new UI consistent with this; when in
doubt, prefer **less** — fewer borders, less color, smaller radii.

## Principles

- **Slim & minimal.** Compact heights (nav is `h-12`), tight spacing, thin `1px` borders.
  No heavy padding or chunky controls.
- **Monochrome first.** Build with the neutral theme tokens. Color is reserved for
  *meaning* — a status dot, a destructive action — never decoration. No gradients in
  chrome (the brand wordmark, buttons, etc.).
- **Sharp, not friendly.** Minimal corner radii. Square avatars/thumbnails; menu items and
  bars are square or only slightly rounded. Avoid `rounded-full`/`rounded-xl` except for
  genuine dots/pills with a reason.
- **Quiet motion.** Subtle `transition-colors`; no bouncy or attention-grabbing animation in
  chrome.

## Theme tokens (don't hardcode colors)

Use the semantic tokens so light/dark both work — never raw `slate-*`/`zinc-*` for chrome:

| Use | Token classes |
|-----|---------------|
| Page surface / text | `bg-background` / `text-foreground` |
| Secondary text | `text-muted-foreground` |
| Subtle fill (cards, hover) | `bg-muted`, `bg-accent` / `text-accent-foreground` |
| Panels / popovers | `bg-card`, `bg-popover` |
| Borders / dividers | `border-border` (e.g. `border-b border-border`, `h-px bg-border`) |
| Destructive | `text-destructive` |
| Focus ring | `focus-visible:ring-ring` |

Accent colors (e.g. `emerald-500` for an online dot) are allowed **only** as small status
signals, not surfaces.

## Dark mode

Class-based: the [`ThemeToggle`](../frontend/src/lib/components/ThemeToggle.svelte) flips a
`dark` class on `<html>` (persisted via `$lib/theme.svelte.ts`; a no-flash script in
`app.html` applies the saved choice before paint). `frontend/src/app.css` defines the `.dark`
token overrides and the `@custom-variant dark`. Because everything uses tokens, components
adapt automatically.

## Components

- **Headless components** — [bits-ui](https://bits-ui.com) (e.g. `DropdownMenu`), styled
  with the theme tokens. Prefer these over hand-rolled controls for overlays/menus.
- **Icons** — Lucide via `@lucide/svelte`: `import { Map, LogOut, Plus } from '@lucide/svelte';`
  then `<Map class="size-4" />`. Size with `size-4`/`size-5`; color via text token.
- **EVE imagery** — [`EveImage`](../frontend/src/lib/components/EveImage.svelte): pass the
  entity `kind` + id + a `class` for size/shape (renders a plain `<img>` from
  `images.evetech.net`). Use square framing (`size-7`, `border-border`).
- **Search palette** — `SystemSearchDialog` (map route): a command-palette modal for picking
  a solar system (driven by a bindable `open` prop + an `onpick` callback), with ↑/↓/Enter/Esc
  keyboard navigation and live server-side search. Reach for this pattern (state-controlled
  overlay) for any searchable picker.

## Layout

- **Full width.** The navbar and every page span the full viewport width — no centered
  `max-w-*` container. Use horizontal padding (`px-5`/`p-6`) for breathing room, not a
  width cap.

## Patterns

- **Navbar** (`frontend/src/lib/components/Nav.svelte`): slim sticky bar, wordmark + text links (`text-muted-foreground`
  → `hover:text-foreground`), right-aligned status + theme toggle + avatar. The avatar opens
  the account dropdown (switch / add / remove character / log out) — no loose buttons in the
  bar.
- **Menu items**: the `MENU_ITEM` style — `flex w-full items-center gap-2 px-2 py-1.5 text-sm
  text-muted-foreground hover:bg-accent hover:text-foreground`; destructive items add
  `hover:text-destructive`.
- **Auth-state UI**: the signed-in character comes from the root layout's server load, so
  it is present on first paint; live data (status, character list) loads client-side.
