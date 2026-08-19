// Throwaway map fixtures, for looking at the canvas under load.
//
// Development only: the menu that reaches this is behind `import.meta.env.DEV` and the
// module is imported dynamically, so none of it ships. Everything here goes through the
// ordinary API, so what it builds is a map like any other and can be cleared like one.

import { toast } from 'svelte-sonner';

import { api } from '$lib/api/client';
import type { MapState } from './map-state.svelte';

/** How the stress chain is shaped: three pinned roots, each growing a branching tree. */
const ROOTS = 3;
const CHILDREN = 2;
const DEPTH = 3;

interface Node {
	/** Index into the placement list, once created. */
	slot: number;
	depth: number;
	row: number;
	parent: number | null;
}

/** The tree, breadth-first, with a row per node so the manual layout is not a pile. */
function shape(): Node[] {
	const nodes: Node[] = [];
	const rows = new Map<number, number>();
	const row = (depth: number) => {
		const next = rows.get(depth) ?? 0;
		rows.set(depth, next + 1);
		return next;
	};
	for (let r = 0; r < ROOTS; r++) {
		nodes.push({ slot: nodes.length, depth: 0, row: row(0), parent: null });
	}
	for (let depth = 1; depth <= DEPTH; depth++) {
		for (const parent of nodes.filter((n) => n.depth === depth - 1)) {
			for (let c = 0; c < CHILDREN; c++) {
				nodes.push({ slot: nodes.length, depth, row: row(depth), parent: parent.slot });
			}
		}
	}
	return nodes;
}

/**
 * Run the work a handful at a time. Sequentially this is a minute of round trips for a
 * map worth stress-testing; all at once it is a burst the dev server has no reason to
 * enjoy.
 */
async function inBatches<T, R>(items: T[], size: number, work: (item: T) => Promise<R>) {
	const out: R[] = [];
	for (let i = 0; i < items.length; i += size) {
		out.push(...(await Promise.all(items.slice(i, i + size).map(work))));
	}
	return out;
}

/** Enough distinct systems to hang the shape off, from the ordinary search. */
async function systemPool(count: number): Promise<number[]> {
	const ids: number[] = [];
	for (const query of ['J1', 'J2', 'J3', 'J4']) {
		if (ids.length >= count) break;
		for (const hit of await api.searchSystems(query)) {
			if (!ids.includes(hit.id)) ids.push(hit.id);
		}
	}
	return ids.slice(0, count);
}

/**
 * Build a chain worth looking at: three pinned roots, a branching tree out of each, and
 * three loops back into it — one between two roots with other roots between them, one
 * across two branches, and one from a leaf back to its own root.
 *
 * The loops are the point. They are the edges a tree layout cannot draw as a tree, so
 * they are where the routing has to prove it stays out of the nodes.
 */
export async function seedStressChain(map: MapState): Promise<void> {
	const nodes = shape();
	const systems = await systemPool(nodes.length);
	if (systems.length < nodes.length) {
		toast.error('debug: not enough systems to build the chain');
		return;
	}

	const placed = await inBatches(nodes, 8, async (node) => {
		const spot = await api.addSystem({
			map_id: map.mapId,
			solar_system_id: systems[node.slot],
			x: 200 + node.depth * 260,
			y: 100 + node.row * 80,
			alias: null
		});
		return spot.id;
	});

	await Promise.all(
		placed
			.slice(0, ROOTS)
			.map((id) => api.setPinned({ map_id: map.mapId, map_solar_system_id: id, value: true }))
	);

	const edges: [number, number][] = nodes
		.filter((n) => n.parent !== null)
		.map((n) => [placed[n.parent!], placed[n.slot]]);

	const leaves = nodes.filter((n) => n.depth === DEPTH).map((n) => n.slot);
	// 1. Two roots with a third between them: the run has to leave the column.
	edges.push([placed[0], placed[2]]);
	// 2. Across two branches, several columns apart.
	edges.push([placed[leaves[0]], placed[leaves[leaves.length - 1]]]);
	// 3. A leaf back to its own root.
	edges.push([placed[leaves[1]], placed[1]]);

	await inBatches(edges, 8, ([from, to]) =>
		api.addConnection({
			map_id: map.mapId,
			from_system: from,
			to_system: to,
			kind: 'wormhole'
		})
	);

	await map.refetch();
}
