// Laying the history tree out as a list of rows, newest at the top.
//
// Indentation tracks divergence, not the cursor: only a step with more than one child forks,
// and then just the sides that were not taken, so rewinding leaves the line straight.
// Rails come from which *line* a row belongs to rather than from its neighbours: two branches
// off the same step are adjacent in the list but belong to different lines.

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
 * Entries whose parent is missing (retention dropped it, or it fell outside the fetched
 * page) are treated as roots, so a truncated history still renders in full.
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

	// Walk oldest-first so a parent is placed before its children; reversed at the end.
	const walk = (parent: number | null, lineage: number[]) => {
		const depth = lineage.length - 1;
		const children = byParent.get(parent) ?? [];
		// A background change is an annotation on the line, not a fork off it.
		for (const note of children.filter((c) => !c.is_step)) {
			nodes.push({ entry: note, depth, forks: false, lineage });
		}

		const steps = children.filter((c) => c.is_step);
		if (steps.length === 0) return;
		// One child carries the line on at this depth: the one the map is on, or the newest
		// when the cursor is rewound past all of them.
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
