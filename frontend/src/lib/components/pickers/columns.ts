// Shared layout for `SystemRow`: class, name, region, holder cell (sovereignty or effect).
//
// The list owns the tracks and every descendant down to the cells is a subgrid
// (`col-span-full grid grid-cols-subgrid`). A grid declared per row would size its columns
// from its own content, so rows in the same list would not line up. Tracks are spelled out
// literally in each container's class so Tailwind sees them at build time. Command-driven
// lists need one extra trailing track for the check indicator `Command.Item` appends, or the
// row wraps onto a second grid line and doubles in height.
export const SYSTEM_ROW_TRACKS = 'min-content minmax(0,1fr) minmax(0,0.8fr) min-content';
/** Number of tracks `SystemRow` occupies. */
export const SYSTEM_ROW_SPAN = 4;
