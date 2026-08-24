// The skyhook card's pure half: rows with their window timing and route, the visible
// subset, and the orderings.

import type { PlanetKind } from '$lib/api/types/PlanetKind';
import type { Skyhook } from '$lib/api/types/Skyhook';
import type { RouteResult } from '$lib/routing/algorithm';
import { timing, type SkyhookStatus } from '$lib/skyhooks/timer';

export interface SkyhookRow {
	skyhook: Skyhook;
	status: SkyhookStatus;
	untilMs: number;
	route: RouteResult | undefined;
	jumps: number | null;
}

export type SkyhookColumn = 'jumps' | 'planet' | 'region' | 'timer';
export const SKYHOOK_COLUMNS = ['jumps', 'planet', 'region', 'timer'] as const;

export function buildSkyhookRows(
	skyhooks: Skyhook[],
	now: Date,
	routes: Map<number, RouteResult>,
): SkyhookRow[] {
	return skyhooks.map((skyhook) => {
		const t = timing(skyhook, now);
		const route = routes.get(skyhook.solar_system_id);
		return { skyhook, status: t.status, untilMs: t.untilMs, route, jumps: route?.jumps ?? null };
	});
}

/** Closed skyhooks are never listed: the window is over, so there is nothing to go and do. */
export function liveSkyhookRows(rows: SkyhookRow[], shown: string[]): SkyhookRow[] {
	return rows.filter((r) => r.status !== 'closed' && shown.includes(r.status));
}

export function skyhookCounts(live: SkyhookRow[]): Record<PlanetKind, number> {
	return {
		lava: live.filter((r) => r.skyhook.planet_kind === 'lava').length,
		ice: live.filter((r) => r.skyhook.planet_kind === 'ice').length,
		other: live.filter((r) => r.skyhook.planet_kind === 'other').length,
	};
}

/** Unreachable sorts last however the column is pointed: it is never the answer. */
function byJumps(a: SkyhookRow, b: SkyhookRow): number {
	if (a.jumps === null || b.jumps === null) return a.jumps === null ? 1 : -1;
	return a.jumps - b.jumps;
}

export const SKYHOOK_COMPARATORS: Record<SkyhookColumn, (a: SkyhookRow, b: SkyhookRow) => number> =
	{
		jumps: byJumps,
		planet: (a, b) => a.skyhook.planet_name.localeCompare(b.skyhook.planet_name),
		region: (a, b) => a.skyhook.region.localeCompare(b.skyhook.region),
		// Open before upcoming, then by how soon the moment is.
		timer: (a, b) => {
			const rank = (r: SkyhookRow) => (r.status === 'upcoming' ? 1 : 0);
			return rank(a) - rank(b) || a.untilMs - b.untilMs;
		},
	};

/** Always break ties the same way, so the order never jitters as timers tick. */
export function skyhookTiebreak(a: SkyhookRow, b: SkyhookRow): number {
	return a.skyhook.planet_id - b.skyhook.planet_id;
}
