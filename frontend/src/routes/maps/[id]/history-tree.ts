// Laying the history tree out as a list of rows.
//
// The path the map is currently on is the trunk and stays at depth 0 however long it gets,
// so an ordinary linear history reads as a plain list. Only a real fork indents, and only
// the side that was abandoned, which keeps a stray undo visible without pushing everything
// that follows it sideways.

import type { MapEventEntry } from '$lib/api/types/MapEventEntry';

export interface HistoryRow {
	entry: MapEventEntry;
	/** Indent level. 0 is the trunk; each abandoned fork adds one. */
	depth: number;
	/** Whether the rail carries on below this row, or stops at its dot. */
	continues: boolean;
}

/**
 * Order the entries parent-first so indentation reads as containment, oldest at the top.
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

	const rows: HistoryRow[] = [];
	const walk = (parent: number | null, depth: number) => {
		const children = byParent.get(parent) ?? [];
		// At most one child carries on the path the map is on; it keeps the trunk's depth.
		const trunk = children.find((c) => c.is_step && c.applied);
		for (const child of children) {
			if (child === trunk) continue;
			if (!child.is_step) {
				// A background change is an annotation on the chain, not a branch off it.
				rows.push({ entry: child, depth, continues: true });
				continue;
			}
			rows.push({ entry: child, depth: depth + 1, continues: true });
			walk(child.id, depth + 1);
		}
		if (trunk) {
			rows.push({ entry: trunk, depth, continues: true });
			walk(trunk.id, depth);
		}
	};
	walk(null, 0);

	// A rail stops at the last row of its group: that is the row after which nothing is
	// nested as deep, so the line has nothing left to connect to.
	rows.forEach((row, i) => {
		const next = rows[i + 1];
		row.continues = next !== undefined && next.depth >= row.depth;
	});
	return rows;
}
