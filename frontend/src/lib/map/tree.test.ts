import { describe, expect, it } from 'vitest';

import { computeTreeLayout, type TreeInput } from './tree';

// Defaults: gridSize 20, levelGap 320, siblingGap 60, marginX 60, marginY 40.

function layout(input: Partial<TreeInput>) {
	return computeTreeLayout({ nodeIds: [], edges: [], rootIds: [], ...input });
}

describe('computeTreeLayout', () => {
	it('returns nothing for an empty map', () => {
		expect(layout({}).size).toBe(0);
	});

	it('lays a chain out left to right, one column per depth', () => {
		const positions = layout({
			nodeIds: [1, 2, 3],
			edges: [
				{ from: 1, to: 2 },
				{ from: 2, to: 3 }
			],
			rootIds: [1]
		});

		expect(positions.get(1)).toEqual({ x: 60, y: 40 });
		expect(positions.get(2)).toEqual({ x: 380, y: 40 });
		expect(positions.get(3)).toEqual({ x: 700, y: 40 });
	});

	it('roots at the home system when nothing is pinned', () => {
		const positions = layout({
			nodeIds: [1, 2],
			edges: [{ from: 1, to: 2 }],
			rootIds: [],
			fallbackRootId: 2
		});

		expect(positions.get(2)!.x).toBe(60);
		expect(positions.get(1)!.x).toBe(380);
	});

	it('fans children out around their parent, a sibling gap apart', () => {
		const positions = layout({
			nodeIds: [1, 2, 3],
			edges: [
				{ from: 1, to: 2 },
				{ from: 1, to: 3 }
			],
			rootIds: [1]
		});

		const parent = positions.get(1)!;
		const first = positions.get(2)!;
		const second = positions.get(3)!;
		expect(first.x).toBe(380);
		expect(second.x).toBe(380);
		expect(second.y - first.y).toBe(60);
		// The parent sits on the snapped midpoint between them.
		expect(Math.abs(parent.y - (first.y + second.y) / 2)).toBeLessThanOrEqual(10);
	});

	it('keeps a tall branch clear of the next one, not just its first row', () => {
		// Two branches off one root; the first has grandchildren, so the second has to
		// clear the whole subtree rather than sitting one gap below its head.
		const positions = layout({
			nodeIds: [1, 2, 3, 4, 5],
			edges: [
				{ from: 1, to: 2 },
				{ from: 2, to: 4 },
				{ from: 2, to: 5 },
				{ from: 1, to: 3 }
			],
			rootIds: [1]
		});

		const deepest = Math.max(positions.get(4)!.y, positions.get(5)!.y);
		expect(positions.get(3)!.y).toBeGreaterThanOrEqual(deepest);
	});

	it('snaps every coordinate to the grid', () => {
		const positions = layout({
			nodeIds: [1, 2, 3, 4, 5, 6, 7],
			edges: [
				{ from: 1, to: 2 },
				{ from: 1, to: 3 },
				{ from: 2, to: 4 },
				{ from: 2, to: 5 },
				{ from: 3, to: 6 },
				{ from: 3, to: 7 }
			],
			rootIds: [1]
		});

		for (const { x, y } of positions.values()) {
			expect(x % 20).toBe(0);
			expect(y % 20).toBe(0);
		}
	});

	it('orders siblings by the comparator it is given', () => {
		const positions = layout({
			nodeIds: [1, 2, 3],
			edges: [
				{ from: 1, to: 2 },
				{ from: 1, to: 3 }
			],
			rootIds: [1],
			compareNodes: (a, b) => b - a
		});

		expect(positions.get(3)!.y).toBeLessThan(positions.get(2)!.y);
	});

	it('parks a system nothing connects to as its own tree', () => {
		const positions = layout({
			nodeIds: [1, 2, 10],
			edges: [{ from: 1, to: 2 }],
			rootIds: [1]
		});

		expect(positions.get(10)!.x).toBe(60);
		expect(positions.get(10)!.y).not.toBe(positions.get(1)!.y);
	});

	it('ignores self-loops and edges to systems that are not on the map', () => {
		const positions = layout({
			nodeIds: [1, 2],
			edges: [
				{ from: 1, to: 1 },
				{ from: 1, to: 99 },
				{ from: 1, to: 2 }
			],
			rootIds: [1]
		});

		expect(positions.size).toBe(2);
		expect(positions.get(2)!.x).toBe(380);
	});

	it('attaches each system to the pinned root nearest it', () => {
		const positions = layout({
			nodeIds: [1, 2, 3, 4],
			edges: [
				{ from: 1, to: 3 },
				{ from: 2, to: 4 },
				{ from: 3, to: 4 }
			],
			rootIds: [1, 2]
		});

		// Both roots in the first column; the 3-4 edge must not drag either deeper.
		expect(positions.get(1)!.x).toBe(60);
		expect(positions.get(2)!.x).toBe(60);
		expect(positions.get(3)!.x).toBe(380);
		expect(positions.get(4)!.x).toBe(380);
	});
});
