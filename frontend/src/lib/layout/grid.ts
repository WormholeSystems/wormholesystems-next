// The panel grid's placement engine: the subset of react-grid-layout's algorithm the layout
// editor needs. Hand-rolled because the Svelte grid libraries all peer on Svelte 4.
//
// Pure and order-stable: same input, same output, and applying a result again is a no-op.

export interface GridItem {
	/** Panel id. Named `i` to match the stored layout shape. */
	i: string;
	x: number;
	y: number;
	w: number;
	h: number;
}

/** The smallest a panel may be made. */
export interface MinSize {
	minW: number;
	minH: number;
}

export function collides(a: GridItem, b: GridItem): boolean {
	if (a.i === b.i) return false;
	return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

/** The first row below every item, i.e. how many rows the layout occupies. */
export function bottom(items: GridItem[]): number {
	return items.reduce((max, item) => Math.max(max, item.y + item.h), 0);
}

/** Reading order: top to bottom, then left to right. Returns the same objects. */
function sorted(items: GridItem[]): GridItem[] {
	return [...items].sort((a, b) => a.y - b.y || a.x - b.x || a.i.localeCompare(b.i));
}

/**
 * Float every item up until it rests on another or the top. Settled in reading order, so
 * the result does not depend on how the items happen to be stored.
 */
export function compact(items: GridItem[], cols: number): GridItem[] {
	const settled: GridItem[] = [];
	for (const item of sorted(items)) {
		const placed = { ...item, w: Math.min(item.w, cols) };
		placed.x = clamp(placed.x, 0, cols - placed.w);
		while (placed.y > 0) {
			const lifted = { ...placed, y: placed.y - 1 };
			if (settled.some((other) => collides(lifted, other))) break;
			placed.y = lifted.y;
		}
		settled.push(placed);
	}
	// Return in the caller's original order so rendering keys stay stable.
	return items.map((item) => settled.find((s) => s.i === item.i)!);
}

/** Put `item` at `(x, y)`, settling whatever it lands on out of the way, then compact. */
export function moveItem(
	items: GridItem[],
	id: string,
	x: number,
	y: number,
	cols: number,
): GridItem[] {
	const moving = items.find((item) => item.i === id);
	if (!moving) return items;
	const placed: GridItem = {
		...moving,
		x: clamp(x, 0, Math.max(0, cols - moving.w)),
		y: Math.max(0, y),
	};
	const rest = items.filter((item) => item.i !== id);
	return compact(push(rest, placed), cols).map((item) => item);
}

/**
 * Resize `item` to `w x h`, clamped to the grid width and the panel's minimum, pushing
 * whatever the new footprint now overlaps.
 */
export function resizeItem(
	items: GridItem[],
	id: string,
	w: number,
	h: number,
	cols: number,
	min: MinSize,
): GridItem[] {
	const target = items.find((item) => item.i === id);
	if (!target) return items;
	const width = clamp(Math.round(w), Math.min(min.minW, cols), cols - target.x);
	const height = Math.max(Math.round(h), min.minH);
	const resized: GridItem = { ...target, w: Math.max(1, width), h: height };
	const rest = items.filter((item) => item.i !== id);
	return compact(push(rest, resized), cols);
}

/**
 * Settle `anchor` among `others`. A displaced item is lifted *above* the anchor when the
 * space it vacated is clear, which is what makes dragging a tile onto its neighbour swap
 * the two rather than appear to do nothing.
 */
function push(others: GridItem[], anchor: GridItem): GridItem[] {
	const result = others.map((item) => ({ ...item }));
	const settled: GridItem[] = [anchor];

	// A pushed item can collide with the next; bounded because each pass settles one more.
	for (let pass = 0; pass < result.length; pass++) {
		const hit = sorted(result).find(
			(item) => !settled.includes(item) && settled.some((s) => collides(item, s)),
		);
		if (!hit) break;
		const blockers = settled.filter((s) => collides(hit, s));
		const topmost = blockers.reduce((top, s) => (s.y < top.y ? s : top));
		const lifted = { ...hit, y: topmost.y - hit.h };
		const clear =
			lifted.y >= 0 &&
			!settled.some((s) => collides(lifted, s)) &&
			!result.some((other) => other !== hit && collides(lifted, other));
		if (clear) {
			hit.y = lifted.y;
		} else {
			// Clear everything settled, not just the first blocker: one drop can land on another.
			let y = hit.y;
			for (;;) {
				const under = settled.filter((s) => collides({ ...hit, y }, s));
				if (under.length === 0) break;
				y = Math.max(...under.map((s) => s.y + s.h));
			}
			hit.y = y;
		}
		settled.push(hit);
	}
	return [...result, anchor];
}

function clamp(v: number, lo: number, hi: number): number {
	return Math.max(lo, Math.min(hi, v));
}

/** Where a resting tile sits, as CSS. */
export interface TileBox {
	left: string;
	top: string;
	width: string;
	height: string;
}

/**
 * A resting tile's box, from the item and the grid alone. Horizontal in percentages, so a
 * tile is painted in the right place before anything has been measured; vertical in pixels,
 * because a row is a fixed height rather than a fraction of one.
 */
export function tileBox(item: GridItem, cols: number, rowHeight: number): TileBox {
	return {
		left: `${(item.x / cols) * 100}%`,
		top: `${item.y * rowHeight}px`,
		width: `${(item.w / cols) * 100}%`,
		height: `${item.h * rowHeight}px`,
	};
}
