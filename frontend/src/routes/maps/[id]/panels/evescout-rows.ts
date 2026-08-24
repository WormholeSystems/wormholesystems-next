// The EVE Scout card's pure half: rows for one hub, and the orderings a scout reads
// them in.

import type { EveScoutConnection } from '$lib/api/types/EveScoutConnection';
import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
import type { RouteResult } from '$lib/routing/algorithm';
import { classMeta } from '$lib/map/classes';

export interface EveScoutRow {
	connection: EveScoutConnection;
	system: SystemSearchResult | undefined;
	route: RouteResult | undefined;
	jumps: number | null;
}

export const EVESCOUT_COLUMNS = ['jumps', 'system', 'region', 'signature', 'type', 'ttl'] as const;
export type EveScoutColumn = (typeof EVESCOUT_COLUMNS)[number];

export function buildEveScoutRows(
	connections: EveScoutConnection[],
	resolve: (id: number) => SystemSearchResult | undefined,
	routes: Map<number, RouteResult>,
): EveScoutRow[] {
	return connections.map((connection) => {
		const route = routes.get(connection.solar_system_id);
		return {
			connection,
			system: resolve(connection.solar_system_id),
			route,
			jumps: route?.jumps ?? null,
		};
	});
}

/** The most recent scout report in the list, or null when nothing carries a stamp. */
export function latestUpdate(connections: EveScoutConnection[]): string | null {
	const stamps = connections.map((c) => c.updated_at).filter((s): s is string => !!s);
	if (stamps.length === 0) return null;
	return stamps.reduce((a, b) => (a > b ? a : b));
}

/** Unreachable sorts last however the column is pointed: it is never the answer. */
function byJumps(a: EveScoutRow, b: EveScoutRow): number {
	if (a.jumps === null && b.jumps === null) return 0;
	if (a.jumps === null || b.jumps === null) return a.jumps === null ? 1 : -1;
	return a.jumps - b.jumps;
}

/**
 * Not alphabetical: known space first by security descending, then wormholes by class,
 * which is the order a scout looks for an exit in.
 */
export function bySystem(a: EveScoutRow, b: EveScoutRow): number {
	const am = classMeta(a.system?.wormhole_class_id ?? null, a.system?.security ?? 0);
	const bm = classMeta(b.system?.wormhole_class_id ?? null, b.system?.security ?? 0);
	if (am.isWormholeSpace !== bm.isWormholeSpace) return am.isWormholeSpace ? 1 : -1;
	if (am.isWormholeSpace) return am.sortWeight - bm.sortWeight;
	return (b.system?.security ?? 0) - (a.system?.security ?? 0);
}

export const EVESCOUT_COMPARATORS: Record<
	EveScoutColumn,
	(a: EveScoutRow, b: EveScoutRow) => number
> = {
	jumps: byJumps,
	system: bySystem,
	region: (a, b) => (a.system?.region ?? '').localeCompare(b.system?.region ?? ''),
	signature: (a, b) => a.connection.hub_signature.localeCompare(b.connection.hub_signature),
	type: (a, b) =>
		(a.connection.wormhole_type ?? '').localeCompare(b.connection.wormhole_type ?? ''),
	// Soonest to collapse first: that is the one you might miss.
	ttl: (a, b) => (a.connection.remaining_hours ?? 999) - (b.connection.remaining_hours ?? 999),
};

/**
 * Ties fall back to class then name, so a column of equal values (no origin, so no jumps)
 * still reads as sorted rather than in EVE Scout's order.
 */
export function eveScoutTiebreak(a: EveScoutRow, b: EveScoutRow): number {
	return bySystem(a, b) || (a.system?.name ?? '').localeCompare(b.system?.name ?? '');
}

/** Hours to something readable at a glance: under a day, minutes matter near the end. */
export function ttl(hours: number | undefined): string {
	if (hours === undefined) return '--';
	if (hours < 1) return `${Math.max(1, Math.round(hours * 60))}m`;
	return `${Math.round(hours)}h`;
}

export function ttlTone(hours: number | undefined): string {
	if (hours === undefined) return 'text-muted-foreground/60';
	if (hours < 1) return 'text-red-500';
	if (hours < 4) return 'text-amber-500';
	return 'text-muted-foreground';
}
