// Documentation for the shared solar-system row layout.
//
// `SystemRow` renders four grid cells: class, name, region, and the holder cell
// (sovereignty logo or effect name).
//
// The rule that makes lists line up: **the list owns the tracks, rows are subgrids**. A
// grid declared on each row sizes its own columns from its own content, so two rows in the
// same list disagree about where the region starts. So the scrolling container carries
// `grid grid-cols-[…]` and every descendant down to the cells carries
// `col-span-full grid grid-cols-subgrid` (or `col-span-N` where something else needs the
// remaining tracks).
//
// The canonical tracks are
// `min-content minmax(0,1fr) minmax(0,0.8fr) min-content` — the class badge and the
// sovereignty/effect cell hug their content while name and region share the slack —
// spelled out literally in each container's class so Tailwind can see them at build time.
// Lists that add their own leading/trailing cells (route index, jump badge, match hint)
// simply prepend/append tracks around these four.
//
// Command-driven lists need one more trailing track: `Command.Item` always appends its own
// check indicator, which would otherwise wrap onto a second grid row and double the row
// height.
export const SYSTEM_ROW_TRACKS = 'min-content minmax(0,1fr) minmax(0,0.8fr) min-content';
/** Number of tracks `SystemRow` occupies. */
export const SYSTEM_ROW_SPAN = 4;
