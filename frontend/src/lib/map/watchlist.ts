// Ordering for the shared watchlist. Kept out of the component so the tie-breaks are
// plain functions of their inputs.

import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
import type { WatchlistEntry } from '$lib/api/types/WatchlistEntry';
import { classMeta } from '$lib/map/classes';

export type WatchlistColumn = 'system' | 'region' | 'jumps';

/**
 * The per-column comparison, before the sort direction is applied. An unresolved system
 * sorts after every resolved one, and an unreachable entry after every reachable one.
 */
export function compareWatchlistEntries(
	a: WatchlistEntry,
	b: WatchlistEntry,
	column: WatchlistColumn,
	info: (id: number) => SystemSearchResult | null,
	jumps: (entry: WatchlistEntry) => number | null,
): number {
	const ra = info(a.solar_system_id);
	const rb = info(b.solar_system_id);
	switch (column) {
		case 'system': {
			const wa = ra ? classMeta(ra.wormhole_class_id, ra.security).sortWeight : 99;
			const wb = rb ? classMeta(rb.wormhole_class_id, rb.security).sortWeight : 99;
			return wa - wb || (ra?.name ?? '').localeCompare(rb?.name ?? '');
		}
		case 'region':
			return (ra?.region ?? '').localeCompare(rb?.region ?? '');
		case 'jumps':
			return (jumps(a) ?? 999) - (jumps(b) ?? 999);
	}
}
