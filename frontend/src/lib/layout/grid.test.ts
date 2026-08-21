import { describe, expect, it } from 'vitest';

import { bottom, collides, compact, moveItem, resizeItem, tileBox, type GridItem } from './grid';

/** Compact form for assertions: `id:x,y,w,h`. */
const show = (items: GridItem[]) =>
	[...items]
		.sort((a, b) => a.i.localeCompare(b.i))
		.map((i) => `${i.i}:${i.x},${i.y},${i.w},${i.h}`);

const item = (i: string, x: number, y: number, w: number, h: number): GridItem => ({
	i,
	x,
	y,
	w,
	h,
});

const MIN = { minW: 1, minH: 1 };

describe('collides', () => {
	it('is false for an item against itself', () => {
		expect(collides(item('a', 0, 0, 2, 2), item('a', 0, 0, 2, 2))).toBe(false);
	});

	it('treats touching edges as clear', () => {
		expect(collides(item('a', 0, 0, 2, 2), item('b', 2, 0, 2, 2))).toBe(false);
		expect(collides(item('a', 0, 0, 2, 2), item('b', 0, 2, 2, 2))).toBe(false);
	});

	it('detects an overlap of a single cell', () => {
		expect(collides(item('a', 0, 0, 2, 2), item('b', 1, 1, 2, 2))).toBe(true);
	});
});

describe('compact', () => {
	it('floats items up into a gap left behind', () => {
		const out = compact([item('a', 0, 0, 2, 2), item('b', 0, 5, 2, 2)], 4);
		expect(show(out)).toEqual(['a:0,0,2,2', 'b:0,2,2,2']);
	});

	it('lets items in different columns rise independently', () => {
		const out = compact([item('a', 0, 0, 2, 4), item('b', 2, 6, 2, 2)], 4);
		expect(show(out)).toEqual(['a:0,0,2,4', 'b:2,0,2,2']);
	});

	it('clamps a too-wide item to the column count', () => {
		const out = compact([item('a', 0, 0, 10, 2)], 4);
		expect(show(out)).toEqual(['a:0,0,4,2']);
	});

	it('pulls an item back inside when it hangs off the right edge', () => {
		const out = compact([item('a', 3, 0, 2, 2)], 4);
		expect(show(out)).toEqual(['a:2,0,2,2']);
	});

	it('is idempotent', () => {
		const once = compact([item('a', 0, 3, 2, 2), item('b', 0, 9, 2, 2)], 4);
		expect(show(compact(once, 4))).toEqual(show(once));
	});

	it('does not depend on the order items are stored in', () => {
		const items = [item('a', 0, 4, 2, 2), item('b', 0, 0, 2, 2), item('c', 2, 0, 2, 3)];
		const forwards = compact(items, 4);
		const backwards = compact([...items].reverse(), 4);
		expect(show(backwards)).toEqual(show(forwards));
	});

	it('preserves the caller order, so render keys stay stable', () => {
		const items = [item('c', 0, 8, 1, 1), item('a', 0, 0, 1, 1), item('b', 0, 4, 1, 1)];
		expect(compact(items, 4).map((i) => i.i)).toEqual(['c', 'a', 'b']);
	});
});

describe('moveItem', () => {
	it('swaps with the neighbour when dragged down onto it', () => {
		const items = [item('a', 0, 0, 2, 2), item('b', 0, 2, 2, 2)];
		const out = moveItem(items, 'a', 0, 2, 4);
		// `b` rises into the space `a` vacated. If it were pushed down instead,
		// compaction would float `a` straight back to the top and the drag would
		// appear to do nothing at all.
		expect(show(out)).toEqual(['a:0,2,2,2', 'b:0,0,2,2']);
	});

	it('clears every item it lands on, not just the first', () => {
		// `c` must end below both `a` and `b`, which sit at different heights.
		const items = [item('a', 0, 0, 2, 2), item('b', 2, 0, 2, 4), item('c', 0, 8, 4, 2)];
		const out = moveItem(items, 'c', 0, 0, 4);
		expect(show(out)).toEqual(['a:0,2,2,2', 'b:2,2,2,4', 'c:0,0,4,2']);
	});

	it('moves an item sideways into a free column', () => {
		const items = [item('a', 0, 0, 2, 2), item('b', 2, 0, 2, 2)];
		const out = moveItem(items, 'a', 2, 2, 4);
		expect(show(out)).toEqual(['a:2,2,2,2', 'b:2,0,2,2']);
	});

	it('cascades a push through a stack', () => {
		const items = [item('a', 0, 0, 4, 2), item('b', 0, 2, 4, 2), item('c', 0, 4, 4, 2)];
		// Drop a tall item over the top of the stack.
		const out = moveItem([...items, item('d', 0, 20, 4, 3)], 'd', 0, 0, 4);
		expect(show(out)).toEqual(['a:0,3,4,2', 'b:0,5,4,2', 'c:0,7,4,2', 'd:0,0,4,3']);
	});

	it('clamps a move past the right edge', () => {
		const out = moveItem([item('a', 0, 0, 2, 2)], 'a', 99, 0, 4);
		expect(show(out)).toEqual(['a:2,0,2,2']);
	});

	it('clamps a move above the top', () => {
		const out = moveItem([item('a', 0, 4, 2, 2)], 'a', 0, -5, 4);
		expect(show(out)).toEqual(['a:0,0,2,2']);
	});

	it('ignores an unknown id', () => {
		const items = [item('a', 0, 0, 2, 2)];
		expect(moveItem(items, 'nope', 2, 2, 4)).toBe(items);
	});
});

describe('resizeItem', () => {
	it('grows an item and pushes what it now overlaps', () => {
		const items = [item('a', 0, 0, 2, 2), item('b', 0, 2, 2, 2)];
		const out = resizeItem(items, 'a', 2, 4, 4, MIN);
		expect(show(out)).toEqual(['a:0,0,2,4', 'b:0,4,2,2']);
	});

	it('clamps width to what is left of the grid', () => {
		const out = resizeItem([item('a', 2, 0, 2, 2)], 'a', 10, 2, 4, MIN);
		expect(show(out)).toEqual(['a:2,0,2,2']);
	});

	it('refuses to shrink below the panel minimum', () => {
		const out = resizeItem([item('a', 0, 0, 4, 4)], 'a', 1, 1, 6, { minW: 3, minH: 2 });
		expect(show(out)).toEqual(['a:0,0,3,2']);
	});

	it('keeps a minimum wider than the grid inside the grid', () => {
		const out = resizeItem([item('a', 0, 0, 1, 1)], 'a', 1, 1, 2, { minW: 5, minH: 1 });
		expect(show(out)).toEqual(['a:0,0,2,1']);
	});
});

describe('bottom', () => {
	it('is the first free row under everything', () => {
		expect(bottom([item('a', 0, 0, 2, 3), item('b', 2, 1, 2, 5)])).toBe(6);
	});

	it('is zero for an empty layout', () => {
		expect(bottom([])).toBe(0);
	});
});

describe('tileBox', () => {
	// The point of the percentages: a tile's resting box is known before anything has been
	// measured, so the first painted frame is already the arrangement.
	it('places a tile from the item and the grid alone', () => {
		expect(tileBox({ i: 'map', x: 0, y: 0, w: 6, h: 4 }, 12, 100)).toEqual({
			left: '0%',
			top: '0px',
			width: '50%',
			height: '400px',
		});
		expect(tileBox({ i: 'notes', x: 9, y: 2, w: 3, h: 1 }, 12, 100)).toEqual({
			left: '75%',
			top: '200px',
			width: '25%',
			height: '100px',
		});
	});
});
