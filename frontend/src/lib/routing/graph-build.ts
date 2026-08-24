// Building the dynamic half of the route graph from the chain, and reading a computed
// path back onto the map's connections. Pure over the map payloads, so the rules are
// testable without a planner.

import type { EveScoutConnection } from '$lib/api/types/EveScoutConnection';
import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { MassStatus } from '$lib/api/types/MassStatus';
import type { TimeStatus } from '$lib/api/types/TimeStatus';
import type { DynamicEdge } from '$lib/routing/algorithm';

/** Placement id -> solar system id, for the placements that are real systems. */
function placementSystems(systems: MapSystemView[]): Map<number, number> {
	const out = new Map<number, number>();
	for (const s of systems) {
		if (s.kind === 'system') out.set(s.id, s.solar_system_id);
	}
	return out;
}

/**
 * The chain's own edges, ready for [`buildDynamicAdjacency`]. Ghosts are left out: a hole
 * whose far side is unknown leads nowhere the router could take you. EVE Scout's public
 * holes ride along only when asked for.
 */
export function chainEdges(
	systems: MapSystemView[],
	connections: MapConnection[],
	eveScout: EveScoutConnection[] | null,
): DynamicEdge[] {
	const placement = placementSystems(systems);
	const edges: DynamicEdge[] = [];
	for (const c of connections) {
		if (c.kind !== 'wormhole') continue;
		const a = placement.get(c.from_system);
		const b = placement.get(c.to_system);
		if (a === undefined || b === undefined || a === b) continue;
		edges.push({ a, b, via: 'wormhole', mass: c.mass_status, time: c.time_status });
	}
	for (const e of eveScout ?? []) {
		edges.push({
			a: e.hub_solar_system_id,
			b: e.solar_system_id,
			via: 'evescout',
			mass: e.mass_status as MassStatus,
			time: e.time_status as TimeStatus,
		});
	}
	return edges;
}

/** Connections on the active route: endpoints at adjacent path indices (legacy rule). */
export function routeConnectionIds(
	path: number[],
	systems: MapSystemView[],
	connections: MapConnection[],
): Set<number> {
	const out = new Set<number>();
	if (path.length < 2) return out;
	const index = new Map<number, number>();
	path.forEach((id, i) => index.set(id, i));
	const placement = placementSystems(systems);
	for (const c of connections) {
		const a = index.get(placement.get(c.from_system) ?? -1);
		const b = index.get(placement.get(c.to_system) ?? -1);
		if (a !== undefined && b !== undefined && Math.abs(a - b) === 1) out.add(c.id);
	}
	return out;
}
