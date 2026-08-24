import { describe, expect, it } from 'vitest';

import type { MapEntry } from '$lib/api/types/MapEntry';
import { byRecency, filterMaps, splitArchived, totalsOf } from './page-model';

const entry = (over: Partial<MapEntry>): MapEntry =>
	({
		name: 'Chain',
		description: null,
		is_archived: false,
		created_at: '2026-08-01T00:00:00Z',
		last_activity: null,
		system_count: 0,
		connection_count: 0,
		pilots_online: 0,
		...over,
	}) as MapEntry;

describe('filterMaps', () => {
	const maps = [entry({ name: 'Home' }), entry({ name: 'Ops', description: 'staging chain' })];

	it('matches name or description, case-insensitively, and blank matches all', () => {
		expect(filterMaps(maps, 'home')).toHaveLength(1);
		expect(filterMaps(maps, 'STAGING')).toHaveLength(1);
		expect(filterMaps(maps, '  ')).toHaveLength(2);
	});
});

describe('byRecency', () => {
	it('puts the most recently touched first, falling back to age', () => {
		const touched = entry({ last_activity: '2026-08-20T00:00:00Z' });
		const untouched = entry({ created_at: '2026-08-10T00:00:00Z' });
		expect(byRecency(touched, untouched)).toBeLessThan(0);
	});
});

describe('splitArchived and totalsOf', () => {
	it('splits, sorts, and counts only the active half', () => {
		const { active, archived } = splitArchived([
			entry({ name: 'A', system_count: 3, pilots_online: 2 }),
			entry({ name: 'B', is_archived: true, system_count: 9 }),
		]);
		expect(active.map((m) => m.name)).toEqual(['A']);
		expect(archived.map((m) => m.name)).toEqual(['B']);
		expect(totalsOf(active)).toEqual({ maps: 1, systems: 3, pilots: 2 });
	});
});
