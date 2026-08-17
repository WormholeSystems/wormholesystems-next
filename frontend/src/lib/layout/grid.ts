// The panel grid's placement engine: the subset of react-grid-layout's algorithm the
// layout editor needs, as pure functions over plain items.
//
// There is no library to lean on here. `svelte-grid-extended` peers on Svelte 4 and
// `svelte-grid` is Svelte 3/4 era, while this project is on Svelte 5 runes.
//
// Everything is pure and order-stable: the same input always yields the same output, and
// applying a result again is a no-op. That is what makes the interactive behaviour
// testable without a browser.

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
 * Float every item up until it rests on another item or the top, so the layout never
 * keeps a gap that a drag left behind.
 *
 * Items are settled in reading order, which is what makes the result independent of the
 * order they happen to be stored in.
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

/**
 * Put `item` at `(x, y)`, settling whatever it lands on out of the way, then compact.
 */
export function moveItem(
	items: GridItem[],
	id: string,
	x: number,
	y: number,
	cols: number
): GridItem[] {
	const moving = items.find((item) => item.i === id);
	if (!moving) return items;
	const placed: GridItem = {
		...moving,
		x: clamp(x, 0, Math.max(0, cols - moving.w)),
		y: Math.max(0, y)
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
	min: MinSize
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
 * Settle `anchor` among `others`, moving anything it overlaps out of the way.
 *
 * A displaced item is lifted *above* the anchor when the space it just vacated is clear,
 * and only pushed below when it is not. That distinction is what makes dragging a tile
 * down onto its neighbour swap the two: pushing unconditionally would send the neighbour
 * down, compaction would float the dragged tile straight back up, and the drag would
 * appear to do nothing.
 */
function push(others: GridItem[], anchor: GridItem): GridItem[] {
	const result = others.map((item) => ({ ...item }));
	const settled: GridItem[] = [anchor];

	// Each pass fixes one item in place; a pushed item can collide with the next, which
	// the following pass picks up. Bounded because every pass settles one more item.
	for (let pass = 0; pass < result.length; pass++) {
		const hit = sorted(result).find(
			(item) => !settled.includes(item) && settled.some((s) => collides(item, s))
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
			// Drop until it clears *everything* settled, not just the first blocker found:
			// landing below one can put it straight on top of another.
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
