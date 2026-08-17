// Laying the history tree out as a list of rows, newest at the top.
//
// Indentation tracks divergence, not the cursor: a step with a single child carries straight
// on at the same depth whether or not the map is currently sitting on it, so rewinding to an
// earlier point leaves the line straight instead of stepping every later change sideways.
// Only a step with more than one child forks, and then just the sides that were not taken.
//
// Rails come from which *line* a row belongs to rather than from its neighbours. Two branches
// off the same step sit next to each other in the list while belonging to different lines, so
// a neighbour-based rule would draw a rail joining them.

import type { MapEventEntry } from '$lib/api/types/MapEventEntry';

export interface HistoryRow {
	entry: MapEventEntry;
	/** Indent level. 0 is a main line; each fork not taken adds one. */
	depth: number;
	/** For each level above this row's own, whether that line's rail passes through here. */
	rails: boolean[];
	/** Whether this row's own line carries on above it. */
	railUp: boolean;
	/** Whether this row's own line carries on below it. */
	railDown: boolean;
	/** Whether this row is where its line left the one it branched from. */
	forks: boolean;
}

interface Node {
	entry: MapEventEntry;
	depth: number;
	forks: boolean;
	/** This row's line, preceded by the line of every level above it. */
	lineage: number[];
}

/**
 * Order the entries newest first, with each row's graph rails resolved.
 *
 * Entries whose parent is missing (retention dropped it, or it fell outside the page we
 * fetched) are treated as roots, so a truncated history still renders in full rather than
 * silently hiding everything below the cut.
 */
export function historyRows(entries: MapEventEntry[]): HistoryRow[] {
	const present = new Set(entries.map((e) => e.id));
	const rootOf = (e: MapEventEntry) =>
		e.parent_id !== null && present.has(e.parent_id) ? e.parent_id : null;

	const byParent = new Map<number | null, MapEventEntry[]>();
	for (const entry of entries) {
		const key = rootOf(entry);
		const siblings = byParent.get(key);
		if (siblings) siblings.push(entry);
		else byParent.set(key, [entry]);
	}
	for (const siblings of byParent.values()) siblings.sort((a, b) => a.id - b.id);

	const nodes: Node[] = [];
	let nextLine = 0;

	// Walk oldest-first so a parent is placed before its children; the list is reversed at
	// the end, which puts the newest work at the top without disturbing the shape.
	const walk = (parent: number | null, lineage: number[]) => {
		const depth = lineage.length - 1;
		const children = byParent.get(parent) ?? [];
		// A background change is an annotation on the line, not a fork off it.
		for (const note of children.filter((c) => !c.is_step)) {
			nodes.push({ entry: note, depth, forks: false, lineage });
		}

		const steps = children.filter((c) => c.is_step);
		if (steps.length === 0) return;
		// One child always carries the line on at this depth. Prefer the one the map is on,
		// so the current path stays straight; with the cursor rewound past all of them,
		// prefer the newest, so the line last worked on is the one that stays straight.
		const main = steps.find((c) => c.applied) ?? steps[steps.length - 1];
		for (const step of steps) {
			if (step === main) continue;
			const branch = [...lineage, nextLine++];
			nodes.push({ entry: step, depth: depth + 1, forks: true, lineage: branch });
			walk(step.id, branch);
		}
		nodes.push({ entry: main, depth, forks: false, lineage });
		walk(main.id, lineage);
	};

	const roots = byParent.get(null) ?? [];
	for (const note of roots.filter((c) => !c.is_step)) {
		nodes.push({ entry: note, depth: 0, forks: false, lineage: [nextLine++] });
	}
	// Each root starts its own line: once retention has split the tree, the surviving
	// fragments are unrelated and must not be drawn as though they join up.
	for (const root of roots.filter((c) => c.is_step)) {
		const lineage = [nextLine++];
		nodes.push({ entry: root, depth: 0, forks: false, lineage });
		walk(root.id, lineage);
	}

	nodes.reverse();

	// A line's rail spans from its first row to its last, so anything nested inside that
	// span draws it as a rail passing through.
	const first = new Map<number, number>();
	const last = new Map<number, number>();
	nodes.forEach((node, i) => {
		const line = node.lineage[node.depth];
		if (!first.has(line)) first.set(line, i);
		last.set(line, i);
	});

	return nodes.map((node, i) => {
		const line = node.lineage[node.depth];
		return {
			entry: node.entry,
			depth: node.depth,
			rails: node.lineage
				.slice(0, node.depth)
				.map((above) => first.get(above)! < i && i < last.get(above)!),
			railUp: first.get(line)! < i,
			railDown: i < last.get(line)!,
			forks: node.forks
		};
	});
}
