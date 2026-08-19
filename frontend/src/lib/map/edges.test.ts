import { describe, expect, it } from 'vitest';

import type { MapConnection } from '$lib/api/types/MapConnection';
import { freeEdges, treeEdges } from './edges';
import { NODE_W } from './helpers';

const NODE_H = 40;

function connection(id: number, from: number, to: number): MapConnection {
	return { id, from_system: from, to_system: to } as MapConnection;
}

describe('freeEdges', () => {
	it('pulls each endpoint along its node toward the other one', () => {
		const positions = new Map([
			[1, { x: 0, y: 0 }],
			[2, { x: 600, y: 0 }]
		]);
		const g = freeEdges([connection(10, 1, 2)], positions, NODE_H).get(10)!;

		expect(g.from.y).toBe(NODE_H / 2);
		expect(g.to.y).toBe(NODE_H / 2);
		expect(g.from.x).toBeGreaterThan(0);
		expect(g.to.x).toBeLessThan(600 + NODE_W);
		expect(g.d.startsWith('M ')).toBe(true);
	});

	it('drops a connection whose endpoint is not placed', () => {
		const positions = new Map([[1, { x: 0, y: 0 }]]);
		expect(freeEdges([connection(10, 1, 2)], positions, NODE_H).size).toBe(0);
	});
});

describe('treeEdges', () => {
	it('leaves the facing sides when the nodes are in different columns', () => {
		const positions = new Map([
			[1, { x: 60, y: 40 }],
			[2, { x: 380, y: 40 }]
		]);
		const g = treeEdges([connection(10, 1, 2)], positions, NODE_H).get(10)!;

		// Out of the right edge of the first, into the left edge of the second.
		expect(g.from).toEqual({ x: 60 + NODE_W, y: 60 });
		expect(g.to).toEqual({ x: 380, y: 60 });
	});

	it('spreads the endpoints of edges that share a node edge', () => {
		const positions = new Map([
			[1, { x: 60, y: 200 }],
			[2, { x: 380, y: 40 }],
			[3, { x: 380, y: 360 }]
		]);
		const routed = treeEdges([connection(10, 1, 2), connection(11, 1, 3)], positions, NODE_H);

		const up = routed.get(10)!;
		const down = routed.get(11)!;
		// Both leave the same side of node 1, so they cannot leave from the same point.
		expect(up.from.x).toBe(down.from.x);
		expect(up.from.y).not.toBe(down.from.y);
		// The one going up leaves above the one going down.
		expect(up.from.y).toBeLessThan(down.from.y);
	});

	it('staggers the vertical runs so a fan nests instead of stacking', () => {
		const positions = new Map([
			[1, { x: 60, y: 200 }],
			[2, { x: 380, y: 40 }],
			[3, { x: 380, y: 120 }],
			[4, { x: 380, y: 360 }]
		]);
		const routed = treeEdges(
			[connection(10, 1, 2), connection(11, 1, 3), connection(12, 1, 4)],
			positions,
			NODE_H
		);

		// The bend is the x of the vertical run; three edges off one node, three lanes.
		const bends = [10, 11, 12].map((id) => {
			const path = routed.get(id)!.d;
			return path;
		});
		expect(new Set(bends).size).toBe(3);
		// Every path is an elbow: it turns, so it has quadratic corners.
		for (const d of bends) expect(d).toContain('Q');
	});

	it('detours into the lane when nodes sit between the two in a column', () => {
		// Four pinned roots in the first column, and a connection joining the outer two:
		// straight down would vanish behind the two in between.
		const positions = new Map([
			[1, { x: 60, y: 40 }],
			[2, { x: 60, y: 160 }],
			[3, { x: 60, y: 280 }],
			[4, { x: 60, y: 400 }]
		]);
		const g = treeEdges([connection(10, 1, 4)], positions, NODE_H).get(10)!;

		// Both ends leave the same side, so the run happens beside the column.
		expect(g.from.x).toBe(60 + NODE_W);
		expect(g.to.x).toBe(60 + NODE_W);
		expect(g.center.x).toBeGreaterThan(60 + NODE_W);
		// And the run clears the nodes it was cutting through.
		expect(g.center.y).toBeGreaterThan(60);
		expect(g.center.y).toBeLessThan(420);
	});

	it('keeps a straight run when the column between them is clear', () => {
		const positions = new Map([
			[1, { x: 60, y: 40 }],
			[2, { x: 60, y: 400 }]
		]);
		const g = treeEdges([connection(10, 1, 2)], positions, NODE_H).get(10)!;

		// Nothing in the way, so it takes the short way: out of the bottom, into the top.
		expect(g.from).toEqual({ x: 60 + NODE_W / 2, y: 80 });
		expect(g.to).toEqual({ x: 60 + NODE_W / 2, y: 400 });
	});

	it('never runs vertically through a column it is only passing', () => {
		// Columns two apart: the midpoint between them is the column in between, which is
		// exactly where a naive bend would put the vertical run.
		const positions = new Map([
			[1, { x: 60, y: 40 }],
			[2, { x: 380, y: 300 }],
			[3, { x: 700, y: 40 }]
		]);
		const g = treeEdges([connection(10, 1, 3)], positions, NODE_H).get(10)!;

		const run = g.center.x;
		expect(run).toBeGreaterThan(60 + NODE_W);
		expect(run < 380 || run > 380 + NODE_W).toBe(true);
	});

	it('goes out of the top or bottom when the nodes share a column', () => {
		const positions = new Map([
			[1, { x: 60, y: 40 }],
			[2, { x: 60, y: 400 }]
		]);
		const g = treeEdges([connection(10, 1, 2)], positions, NODE_H).get(10)!;

		expect(g.from).toEqual({ x: 60 + NODE_W / 2, y: 80 });
		expect(g.to).toEqual({ x: 60 + NODE_W / 2, y: 400 });
	});
});
