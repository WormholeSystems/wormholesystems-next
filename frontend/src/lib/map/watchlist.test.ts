import { describe, expect, it } from 'vitest';

import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
import type { WatchlistEntry } from '$lib/api/types/WatchlistEntry';
import { compareWatchlistEntries } from './watchlist';

const entry = (id: number): WatchlistEntry => ({ solar_system_id: id }) as WatchlistEntry;
const result = (over: Partial<SystemSearchResult>): SystemSearchResult =>
	({ wormhole_class_id: null, security: 1, name: '', region: '', ...over }) as SystemSearchResult;

const INFO: Record<number, SystemSearchResult> = {
	1: result({ name: 'Jita', region: 'The Forge', security: 0.9 }),
	2: result({ name: 'J155207', region: 'D-R00018', wormhole_class_id: 5, security: -1 }),
	3: result({ name: 'Amarr', region: 'Domain', security: 1.0 }),
};
const info = (id: number) => INFO[id] ?? null;

describe('compareWatchlistEntries', () => {
	it('sorts known space before wormhole space by class weight, then by name', () => {
		expect(compareWatchlistEntries(entry(1), entry(2), 'system', info, () => null)).toBeLessThan(0);
		expect(compareWatchlistEntries(entry(3), entry(1), 'system', info, () => null)).toBeLessThan(0);
	});

	it('sorts an unresolved system after every resolved one', () => {
		expect(compareWatchlistEntries(entry(2), entry(99), 'system', info, () => null)).toBeLessThan(
			0,
		);
	});

	it('sorts by region name alphabetically', () => {
		expect(compareWatchlistEntries(entry(3), entry(1), 'region', info, () => null)).toBeLessThan(0);
	});

	it('sorts unreachable entries after reachable ones by jumps', () => {
		const jumps = (e: WatchlistEntry) => (e.solar_system_id === 1 ? 4 : null);
		expect(compareWatchlistEntries(entry(1), entry(2), 'jumps', info, jumps)).toBeLessThan(0);
	});
});
