// The context menu's shared look: rows, submenu triggers, and the CSS-hover flyout panels
// they open. One place, so the three menus cannot drift apart.

export const item =
	'flex w-full items-center gap-2 px-3 py-1 text-left text-xs text-foreground hover:bg-accent';

export const sub =
	'relative group/sub flex w-full cursor-default items-center gap-2 px-3 py-1 text-left text-xs text-foreground hover:bg-accent';

export const panel =
	'absolute left-full top-0 z-40 hidden min-w-40 border border-border bg-popover py-1 shadow-md group-hover/sub:block';
