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
