// Client-side route computation, ported from the legacy routing worker: Dijkstra with a
// binary heap over the static stargate graph plus the map's live wormhole connections.
// Zarzakh is always excluded.

export type RoutePreference = 'shorter' | 'safer' | 'less_secure';

export interface RouteStep {
	id: number;
	via: 'stargate' | 'wormhole' | null;
}

export interface RouteResult {
	route: RouteStep[];
	jumps: number;
	cost: number;
}

export interface RoutingSettings {
	preference: RoutePreference;
	/** 0-100, weight of the security penalty in safer/less-secure modes. */
	securityPenalty: number;
}

export const ZARZAKH_SYSTEM_ID = 30100000;

type Adjacency = Map<number, number[]>;

class PriorityQueue {
	private heap: { id: number; cost: number }[] = [];

	push(id: number, cost: number) {
		this.heap.push({ id, cost });
		let i = this.heap.length - 1;
		while (i > 0) {
			const parent = (i - 1) >> 1;
			if (this.heap[parent].cost <= this.heap[i].cost) break;
			[this.heap[parent], this.heap[i]] = [this.heap[i], this.heap[parent]];
			i = parent;
		}
	}

	pop(): { id: number; cost: number } | undefined {
		const top = this.heap[0];
		const last = this.heap.pop();
		if (this.heap.length > 0 && last) {
			this.heap[0] = last;
			let i = 0;
			for (;;) {
				const l = 2 * i + 1;
				const r = 2 * i + 2;
				let smallest = i;
				if (l < this.heap.length && this.heap[l].cost < this.heap[smallest].cost) smallest = l;
				if (r < this.heap.length && this.heap[r].cost < this.heap[smallest].cost) smallest = r;
				if (smallest === i) break;
				[this.heap[smallest], this.heap[i]] = [this.heap[i], this.heap[smallest]];
				i = smallest;
			}
		}
		return top;
	}

	get size() {
		return this.heap.length;
	}
}

/** Per-target-system edge cost, matching the legacy cost function. */
function edgeCost(
	settings: RoutingSettings,
	targetSecurity: number | undefined
): number {
	if (settings.preference === 'shorter') return 1;
	const penalty = Math.exp(0.15 * settings.securityPenalty);
	const sec = targetSecurity ?? 0;
	if (settings.preference === 'safer') {
		if (sec <= 0) return 2 * penalty;
		if (sec < 0.45) return penalty;
		return 0.9;
	}
	// less_secure
	if (sec <= 0) return 2 * penalty;
	if (sec < 0.45) return 0.9;
	return penalty;
}

export interface RouteGraph {
	/** Static stargate adjacency (both directions present). */
	stargates: Adjacency;
	/** Live wormhole adjacency from the map's connections (both directions present). */
	wormholes: Adjacency;
	/** Security by solar system id, for the safer / less-secure cost functions. */
	security: Map<number, number>;
}

export function buildAdjacency(edges: [number, number][]): Adjacency {
	const adj: Adjacency = new Map();
	const add = (a: number, b: number) => {
		if (a === b) return;
		const list = adj.get(a) ?? [];
		if (!list.includes(b)) list.push(b);
		adj.set(a, list);
	};
	for (const [a, b] of edges) {
		add(a, b);
		add(b, a);
	}
	return adj;
}

export function findRoute(
	graph: RouteGraph,
	from: number,
	to: number,
	settings: RoutingSettings,
	ignored: ReadonlySet<number> = new Set()
): RouteResult | null {
	if (from === to) return { route: [{ id: from, via: null }], jumps: 0, cost: 0 };

	const isIgnored = (id: number) =>
		id !== from && id !== to && (id === ZARZAKH_SYSTEM_ID || ignored.has(id));

	const dist = new Map<number, number>();
	const prev = new Map<number, { id: number; via: 'stargate' | 'wormhole' }>();
	const queue = new PriorityQueue();
	dist.set(from, 0);
	queue.push(from, 0);

	while (queue.size > 0) {
		const current = queue.pop()!;
		if (current.id === to) break;
		if (current.cost > (dist.get(current.id) ?? Infinity)) continue;

		const expand = (neighbors: number[] | undefined, via: 'stargate' | 'wormhole') => {
			for (const next of neighbors ?? []) {
				if (isIgnored(next)) continue;
				const cost = current.cost + edgeCost(settings, graph.security.get(next));
				if (cost < (dist.get(next) ?? Infinity)) {
					dist.set(next, cost);
					prev.set(next, { id: current.id, via });
					queue.push(next, cost);
				}
			}
		};
		expand(graph.stargates.get(current.id), 'stargate');
		expand(graph.wormholes.get(current.id), 'wormhole');
	}

	if (!dist.has(to)) return null;

	// Reconstruct; each step carries the means of ARRIVING at it.
	const route: RouteStep[] = [];
	let cursor: number | undefined = to;
	while (cursor !== undefined) {
		const p = prev.get(cursor);
		route.push({ id: cursor, via: p?.via ?? null });
		cursor = p?.id;
	}
	route.reverse();
	return { route, jumps: Math.max(0, route.length - 1), cost: dist.get(to)! };
}
