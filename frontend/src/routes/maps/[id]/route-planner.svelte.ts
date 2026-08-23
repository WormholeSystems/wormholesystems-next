// Route planning over the chain: the static universe tables, the graph derived from them
// plus the map's own edges, and the pinned A→B route with its hover override.

import { api } from '$lib/api/client';
import type { MassStatus } from '$lib/api/types/MassStatus';
import type { TimeStatus } from '$lib/api/types/TimeStatus';
import type { DynamicEdge, RouteGraph, RouteStep } from '$lib/routing/algorithm';
import { buildDynamicAdjacency } from '$lib/routing/algorithm';
import { solarSystemId } from '$lib/map/system';
import * as v from 'valibot';
import { readStored } from '$lib/storage';
import type { MapState } from './map-state.svelte';

/** Somewhere worth routing to, and the stations that make it worth it. */
export interface StationGroup {
	id: number;
	name: string;
	systems: Set<number>;
	/** Concrete stations per system, so results can name (and target) the station. */
	stationsBySystem: Map<number, { id: number; name: string }[]>;
}

function stationGroup(group: {
	id: number;
	name: string;
	stations: { id: number; name: string; solar_system_id: number }[];
}): StationGroup {
	const stationsBySystem = new Map<number, { id: number; name: string }[]>();
	for (const station of group.stations) {
		const list = stationsBySystem.get(station.solar_system_id) ?? [];
		list.push({ id: station.id, name: station.name });
		stationsBySystem.set(station.solar_system_id, list);
	}
	return {
		id: group.id,
		name: group.name,
		systems: new Set(stationsBySystem.keys()),
		stationsBySystem,
	};
}

export class RoutePlanner {
	private map: MapState;

	// Origin/destination (solar system ids) and the computed path, set by the navigation
	// card. The path drives the edge highlight.
	routeFromId = $state<number | null>(null);
	routeToId = $state<number | null>(null);
	routePath = $state<number[]>([]);
	// A route hovered in a side panel temporarily replaces the pinned A→B highlight.
	hoverPath = $state<number[] | null>(null);
	// Systems the router steers around (per map, persisted locally).
	ignoredSystems = $state<Set<number>>(new Set());

	// The static routing data, fetched once and shared: the navigation card plans routes
	// with it, and the pilots card measures distances with it. One home, one fetch.
	stargates = $state<Map<number, number[]> | null>(null);
	security = $state<Map<number, number>>(new Map());
	joveSystems = $state<Set<number>>(new Set());
	stationSystems = $state<Set<number>>(new Set());
	/** A named set of stations to search for: what a station does, or who owns it. */
	serviceOptions = $state<StationGroup[]>([]);
	corporationOptions = $state<StationGroup[]>([]);

	constructor(map: MapState) {
		this.map = map;
	}

	/** Stargates plus the chain's own edges. `null` until the static data has arrived. */
	graph = $derived.by<RouteGraph | null>(() => {
		const stargates = this.stargates;
		if (!stargates) return null;
		// Ghosts are left out: a hole whose far side is unknown leads nowhere the router
		// could take you.
		const placementSystem = new Map<number, number>();
		for (const s of this.map.systems) {
			if (s.kind === 'system') placementSystem.set(s.id, s.solar_system_id);
		}
		const edges: DynamicEdge[] = [];
		for (const c of this.map.connections) {
			if (c.kind !== 'wormhole') continue;
			const a = placementSystem.get(c.from_system);
			const b = placementSystem.get(c.to_system);
			if (a === undefined || b === undefined || a === b) continue;
			edges.push({ a, b, via: 'wormhole', mass: c.mass_status, time: c.time_status });
		}
		if (this.map.useEveScout) {
			for (const e of this.map.eveScout) {
				edges.push({
					a: e.hub_solar_system_id,
					b: e.solar_system_id,
					via: 'evescout',
					mass: e.mass_status as MassStatus,
					time: e.time_status as TimeStatus,
				});
			}
		}
		return { stargates, dynamic: buildDynamicAdjacency(edges), security: this.security };
	});

	// Connections on the active route: endpoints at adjacent path indices (legacy rule).
	routeConnectionIds = $derived.by(() => {
		const out = new Set<number>();
		if (this.routePath.length < 2) return out;
		const index = new Map<number, number>();
		this.routePath.forEach((id, i) => index.set(id, i));
		const placementSystem = new Map<number, number>();
		for (const s of this.map.systems) {
			if (s.kind === 'system') placementSystem.set(s.id, s.solar_system_id);
		}
		for (const c of this.map.connections) {
			const a = index.get(placementSystem.get(c.from_system) ?? -1);
			const b = index.get(placementSystem.get(c.to_system) ?? -1);
			if (a !== undefined && b !== undefined && Math.abs(a - b) === 1) out.add(c.id);
		}
		return out;
	});

	private ignoreStorageKey(): string {
		return `route-ignored-${this.map.mapId}`;
	}

	loadIgnored() {
		this.ignoredSystems = new Set(readStored(this.ignoreStorageKey(), v.array(v.number()), []));
	}

	ignoreSystem(id: number) {
		const next = new Set(this.ignoredSystems);
		next.add(id);
		this.ignoredSystems = next;
		localStorage.setItem(this.ignoreStorageKey(), JSON.stringify([...next]));
	}

	clearIgnored() {
		this.ignoredSystems = new Set();
		localStorage.removeItem(this.ignoreStorageKey());
	}

	/** The static routing tables (stargates, security, Jove/station/service indexes). */
	private routingLoad: Promise<void> | null = null;

	/**
	 * Resolves once the static routing tables are in. Jump tracking waits on this rather
	 * than reading `stargates` directly, since a jump taken seconds after the map opens
	 * would otherwise be judged against an empty gate table.
	 */
	whenLoaded(): Promise<void> {
		return this.routingLoad ?? this.load();
	}

	async load() {
		this.routingLoad ??= this.fetchGraph();
		return this.routingLoad;
	}

	private async fetchGraph() {
		try {
			const g = await api.routingGraph();
			this.stargates = new Map(
				Object.entries(g.adjacency).map(([k, v]) => [Number(k), v as number[]]),
			);
			this.security = new Map(Object.entries(g.security).map(([k, v]) => [Number(k), v]));
			this.joveSystems = new Set(g.jove ?? []);
			this.stationSystems = new Set(g.stations ?? []);
			this.serviceOptions = (g.services ?? []).map(stationGroup);
			this.corporationOptions = (g.corporations ?? []).map(stationGroup);
		} catch {
			// No graph means no routing; the cards fall back to showing no distances.
		}
	}

	/**
	 * The signature to warp to for a wormhole hop. A connection has one at each end; the
	 * one that matters is on the side you are leaving, which is the one in your scanner.
	 */
	wormholeSignature(from: number, to: number): string | null {
		const system = new Map(this.map.systems.map((s) => [s.id, solarSystemId(s)]));
		const conn = this.map.connections.find((c) => {
			const a = system.get(c.from_system);
			const b = system.get(c.to_system);
			return (a === from && b === to) || (a === to && b === from);
		});
		if (!conn) return null;
		return (
			this.map.sigs.find((sig) => sig.connection_id === conn.id && sig.solar_system_id === from)
				?.signature_id ?? null
		);
	}

	/** Route steps with the signature attached to each wormhole hop. */
	withSignatures(steps: RouteStep[]): (RouteStep & { signature: string | null })[] {
		return steps.map((step, i) => ({
			...step,
			signature:
				step.via === 'wormhole' && i > 0 ? this.wormholeSignature(steps[i - 1].id, step.id) : null,
		}));
	}
}
