import { describe, expect, it } from 'vitest';

import type { RoutingGraph } from '$lib/api/types/RoutingGraph';
import { RoutePlanner, type RouteHost } from './route-planner.svelte';

const TABLES: RoutingGraph = {
	adjacency: { '100': [200], '200': [100] },
	security: { '100': 0.9, '200': 0.4 },
	jove: [300],
	stations: [100],
	services: [{ id: 1, name: 'Cloning', stations: [{ id: 5, name: 'Home', solar_system_id: 100 }] }],
	corporations: [],
};

function host(over: Partial<RouteHost> = {}): RouteHost {
	return {
		mapId: 1,
		systems: () => [],
		connections: () => [],
		sigs: () => [],
		eveScout: () => [],
		useEveScout: () => false,
		loadTables: () => Promise.resolve(TABLES),
		...over,
	};
}

describe('RoutePlanner', () => {
	it('has no graph until the tables arrive', async () => {
		const planner = new RoutePlanner(host());
		expect(planner.graph).toBeNull();
		await planner.load();
		expect(planner.graph).not.toBeNull();
		expect(planner.stargates?.get(100)).toEqual([200]);
		expect(planner.joveSystems.has(300)).toBe(true);
		expect(planner.serviceOptions[0]?.systems.has(100)).toBe(true);
	});

	it('fetches the tables once however many callers wait on it', async () => {
		let calls = 0;
		const planner = new RoutePlanner(
			host({
				loadTables: () => {
					calls += 1;
					return Promise.resolve(TABLES);
				},
			}),
		);
		await Promise.all([planner.whenLoaded(), planner.load(), planner.whenLoaded()]);
		expect(calls).toBe(1);
	});

	it('degrades to no routing when the tables fail to load', async () => {
		const planner = new RoutePlanner(host({ loadTables: () => Promise.reject(new Error('down')) }));
		await planner.load();
		expect(planner.graph).toBeNull();
		expect(planner.stargates).toBeNull();
	});

	it('keeps the ignore list as a set that grows and clears', () => {
		const planner = new RoutePlanner(host());
		planner.ignoreSystem(100);
		planner.ignoreSystem(200);
		expect(planner.ignoredSystems).toEqual(new Set([100, 200]));
		planner.clearIgnored();
		expect(planner.ignoredSystems.size).toBe(0);
	});
});
