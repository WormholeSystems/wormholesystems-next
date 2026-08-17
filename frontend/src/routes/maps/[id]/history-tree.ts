// Laying the history tree out as a list of rows.
//
// Indentation tracks divergence, not the cursor: a step with a single child carries straight
// on at the same depth whether or not the map is currently sitting on it, so rewinding to an
// earlier point leaves the line straight instead of stepping every later change sideways.
// Only a step with more than one child forks, and then just the sides that were not taken.

import type { MapEventEntry } from '$lib/api/types/MapEventEntry';

export interface HistoryRow {
	entry: MapEventEntry;
	/** Indent level. 0 is the main line; each fork not taken adds one. */
	depth: number;
	/** Whether the rail carries on below this row, or stops at its dot. */
	continues: boolean;
	/**
	 * Whether this row starts its indent level, so it is the one that connects back to the
	 * parent's rail. Rows below it on the same branch just carry their own line down.
	 */
	forks: boolean;
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
		// A background change is an annotation on the line, not a fork off it.
		for (const note of children.filter((c) => !c.is_step)) {
			rows.push({ entry: note, depth, continues: true, forks: false });
		}

		const steps = children.filter((c) => c.is_step);
		if (steps.length === 0) return;
		// One child always carries the line on at this depth. Prefer the one the map is on,
		// so the current path stays straight; with the cursor rewound past all of them,
		// prefer the newest, so the line people last worked on is the one that stays straight.
		const main = steps.find((c) => c.applied) ?? steps[steps.length - 1];
		for (const step of steps) {
			if (step === main) continue;
			// Only this row hangs off the parent's rail; whatever follows it on the branch is
			// reached by walking down from here, at the same depth.
			rows.push({ entry: step, depth: depth + 1, continues: true, forks: true });
			walk(step.id, depth + 1);
		}
		rows.push({ entry: main, depth, continues: true, forks: false });
		walk(main.id, depth);
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
