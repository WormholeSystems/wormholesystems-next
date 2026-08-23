import { describe, expect, it } from 'vitest';

import type { MapEventEntry } from '$lib/api/types/MapEventEntry';
import { historyRows } from './history-tree';

const entry = (
	id: number,
	parent: number | null,
	over: Partial<MapEventEntry> = {},
): MapEventEntry => ({
	id,
	map_id: 1,
	character_id: null,
	character_name: null,
	kind: 'test',
	label: `event ${id}`,
	entries_count: 1,
	parent_id: parent,
	is_step: true,
	applied: true,
	created_at: '2026-08-17T15:00:00Z',
	...over,
});

const ids = (rows: ReturnType<typeof historyRows>) => rows.map((r) => r.entry.id);

describe('historyRows', () => {
	it('renders nothing from nothing', () => {
		expect(historyRows([])).toEqual([]);
	});

	it('lays a straight line out newest first, at depth 0, with the rail through it', () => {
		const rows = historyRows([entry(1, null), entry(2, 1), entry(3, 2)]);
		expect(ids(rows)).toEqual([3, 2, 1]);
		expect(rows.every((r) => r.depth === 0)).toBe(true);
		expect(rows.every((r) => !r.forks)).toBe(true);
		// Top row: line continues below; bottom row: line continues above.
		expect(rows[0]).toMatchObject({ railUp: false, railDown: true });
		expect(rows[1]).toMatchObject({ railUp: true, railDown: true });
		expect(rows[2]).toMatchObject({ railUp: true, railDown: false });
	});

	it('indents only the side not taken, and marks where it forked', () => {
		// 1 -> 2 (applied) and 1 -> 3 (abandoned branch).
		const rows = historyRows([entry(1, null), entry(2, 1), entry(3, 1, { applied: false })]);
		const main = rows.filter((r) => r.entry.id !== 3);
		const branch = rows.find((r) => r.entry.id === 3)!;
		expect(main.every((r) => r.depth === 0)).toBe(true);
		expect(branch.depth).toBe(1);
		expect(branch.forks).toBe(true);
		// The abandoned side is a single-row line: no rail of its own in either direction.
		expect(branch).toMatchObject({ railUp: false, railDown: false });
	});

	it('draws the trunk rail as passing through the rows of a nested branch', () => {
		// A branch with two rows nested inside the trunk's span.
		const rows = historyRows([
			entry(1, null),
			entry(2, 1),
			entry(3, 1, { applied: false }),
			entry(4, 3, { applied: false }),
		]);
		expect(ids(rows)).toEqual([2, 4, 3, 1]);
		const inBranch = rows.filter((r) => r.entry.id === 4 || r.entry.id === 3);
		for (const row of inBranch) {
			expect(row.depth).toBe(1);
			// Level 0 is the trunk, still open above and below these rows.
			expect(row.rails).toEqual([true]);
		}
	});

	it('keeps a background note on its line without forking', () => {
		const rows = historyRows([entry(1, null), entry(2, 1, { is_step: false }), entry(3, 1)]);
		const note = rows.find((r) => r.entry.id === 2)!;
		expect(note.depth).toBe(0);
		expect(note.forks).toBe(false);
	});

	it('treats an entry whose parent was retained away as its own root', () => {
		// Parent 99 is not in the list: 5 and its child must still render, unjoined to 1.
		const rows = historyRows([entry(1, null), entry(5, 99), entry(6, 5)]);
		expect(ids(rows)).toEqual([6, 5, 1]);
		expect(rows.every((r) => r.depth === 0)).toBe(true);
		// Two fragments, so no rail joins row "5" down to row "1".
		expect(rows.find((r) => r.entry.id === 5)!.railDown).toBe(false);
	});
});
