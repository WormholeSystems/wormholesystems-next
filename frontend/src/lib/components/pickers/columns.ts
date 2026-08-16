// Documentation for the shared solar-system row layout.
//
// `SystemRow` renders four grid cells: class, name, region, and the holder cell
// (sovereignty logo or effect name). Every list that uses it declares matching tracks
// on its container and marks each row `col-span-full grid grid-cols-subgrid`, so all
// rows align and the columns resize with the container instead of using fixed widths.
//
// The canonical tracks are
// `min-content minmax(0,1fr) minmax(0,0.8fr) min-content` — the class badge and the
// sovereignty/effect cell hug their content while name and region share the slack —
// spelled out
// literally in each container's class so Tailwind can see them at build time. Lists
// that add their own leading/trailing cells (route index, jump badge, actions) simply
// prepend/append tracks around these four.
export const SYSTEM_ROW_TRACKS = 'min-content minmax(0,1fr) minmax(0,0.8fr) min-content';
/** Number of tracks `SystemRow` occupies. */
export const SYSTEM_ROW_SPAN = 4;
