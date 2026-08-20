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

describe('a hub with many children in the next column', () => {
	const NODE_H2 = 40;
	const targetX = NODE_W + 136;
	const ys = [20, 80, 147, 207, 267, 327, 387, 447, 507, 567, 627, 687, 747];

	function hub() {
		const positions = new Map([[1, { x: 0, y: 390 }]]);
		ys.forEach((y, i) => positions.set(i + 2, { x: targetX, y }));
		return treeEdges(
			ys.map((_, i) => connection(10 + i, 1, i + 2)),
			positions,
			NODE_H2
		);
	}

	// The bend used to step a fixed distance from the middle, which walked the outermost
	// runs onto the target column: they were then shunted past it and doubled back.
	it('keeps every run between the two columns', () => {
		for (const edge of hub().values()) {
			for (const [, x] of edge.d.matchAll(/[MLQ] (-?[\d.]+) -?[\d.]+/g)) {
				expect(Number(x)).toBeGreaterThanOrEqual(NODE_W - 0.5);
				expect(Number(x)).toBeLessThanOrEqual(targetX + 0.5);
			}
		}
	});

	it('nests the runs so none of them cross', () => {
		const segments = [...hub().values()].map((e) => {
			const pts = [...e.d.matchAll(/[ML] (-?[\d.]+) (-?[\d.]+)/g)].map((m) => ({
				x: Number(m[1]),
				y: Number(m[2])
			}));
			return pts.slice(1).map((p, i) => ({ x1: pts[i].x, y1: pts[i].y, x2: p.x, y2: p.y }));
		});

		let crossings = 0;
		for (let i = 0; i < segments.length; i++) {
			for (let j = i + 1; j < segments.length; j++) {
				for (const a of segments[i]) {
					for (const b of segments[j]) {
						const aFlat = Math.abs(a.y1 - a.y2) < 0.5;
						if (aFlat === Math.abs(b.y1 - b.y2) < 0.5) continue;
						const [h, v] = aFlat ? [a, b] : [b, a];
						const spansX = Math.min(h.x1, h.x2) < v.x1 - 0.5 && v.x1 < Math.max(h.x1, h.x2) - 0.5;
						const spansY = Math.min(v.y1, v.y2) < h.y1 - 0.5 && h.y1 < Math.max(v.y1, v.y2) - 0.5;
						if (spansX && spansY) crossings++;
					}
				}
			}
		}
		expect(crossings).toBe(0);
	});
});

describe('runs share a lane where they can', () => {
	const NODE_H2 = 40;
	const COL = NODE_W + 136;

	function bendsOf(edges: ReturnType<typeof treeEdges>) {
		return [...edges.values()].map((e) => {
			const xs = [...e.d.matchAll(/[ML] (-?[\d.]+) /g)].map((m) => Number(m[1]));
			return Math.round(xs[1]);
		});
	}

	// A run up and a run down never overlap, so they can sit on the same line and still be
	// told apart: each keeps its own stroke. Giving them separate lines made a two-hole
	// system look like a ladder.
	it('puts one child above and one below on the same line', () => {
		const positions = new Map([
			[1, { x: 0, y: 300 }],
			[2, { x: COL, y: 180 }],
			[3, { x: COL, y: 420 }]
		]);
		const bends = bendsOf(treeEdges([connection(10, 1, 2), connection(11, 1, 3)], positions, NODE_H2));
		expect(new Set(bends).size).toBe(1);
	});

	// Two runs the same way do overlap, and would hide each other.
	it('keeps two children on the same side apart', () => {
		const positions = new Map([
			[1, { x: 0, y: 300 }],
			[2, { x: COL, y: 60 }],
			[3, { x: COL, y: 180 }]
		]);
		const bends = bendsOf(treeEdges([connection(10, 1, 2), connection(11, 1, 3)], positions, NODE_H2));
		expect(new Set(bends).size).toBe(2);
	});
});
