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

Class-based: the [`ThemeToggle`](../src/components/ui/theme_toggle.rs) flips a `dark` class on
`<html>` (synced from `ThemeMode` in `app/mod.rs`; a no-flash script in the shell applies the
saved choice before paint). `style/tailwind.css` defines the `.dark` token overrides and the
`@custom-variant dark`. Because everything uses tokens, components adapt automatically.

## Components

- **Vendored Rust/UI library** — `crate::components::ui::*` (added via the `ui` CLI:
  `ui add <name>`). Buttons, dialog, dropdown_menu, popover, command, input, avatar, etc.
  Prefer these over hand-rolled controls.
- **Icons** — Lucide via the `icons` crate: `use icons::{Map, LogOut, Plus};` then
  `<Map class="size-4" />`. Size with `size-4`/`size-5`; color via text token.
- **EVE imagery** — `crate::app::components::{CharacterImage, CorporationImage,
  AllianceImage, TypeImage}`. Pass the entity id + a `class` for size/shape (they render a
  plain `<img>` from `images.evetech.net`). Use square framing (`size-7`, `border-border`).

## Layout

- **Full width.** The navbar and every page span the full viewport width — no centered
  `max-w-*` container. Use horizontal padding (`px-5`/`p-6`) for breathing room, not a
  width cap.

## Patterns

- **Navbar** (`app/mod.rs`): slim sticky bar, wordmark + text links (`text-muted-foreground`
  → `hover:text-foreground`), right-aligned status + theme toggle + avatar. The avatar opens
  the account dropdown (switch / add / remove character / log out) — no loose buttons in the
  bar.
- **Menu items**: the `MENU_ITEM` style — `flex w-full items-center gap-2 px-2 py-1.5 text-sm
  text-muted-foreground hover:bg-accent hover:text-foreground`; destructive items add
  `hover:text-destructive`.
- **Auth-state UI** lives in one `<Suspense>`/`<Transition>` boundary; never nest async
  boundaries (causes hydration mismatches) — see the navbar.
