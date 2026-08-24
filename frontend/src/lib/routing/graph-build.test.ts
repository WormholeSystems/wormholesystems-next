import { describe, expect, it } from 'vitest';

import type { EveScoutConnection } from '$lib/api/types/EveScoutConnection';
import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import { chainEdges, routeConnectionIds } from './graph-build';

const system = (id: number, solarSystemId: number): MapSystemView =>
	({ kind: 'system', id, solar_system_id: solarSystemId }) as MapSystemView;
const ghost = (id: number): MapSystemView => ({ kind: 'ghost', id }) as MapSystemView;
const conn = (id: number, from: number, to: number, kind = 'wormhole'): MapConnection =>
	({
		id,
		from_system: from,
		to_system: to,
		kind,
		mass_status: 'stable',
		time_status: 'stable',
	}) as MapConnection;

const SYSTEMS = [system(1, 31000001), system(2, 31000002), ghost(3)];

describe('chainEdges', () => {
	it('turns wormhole connections between real systems into edges', () => {
		const edges = chainEdges(SYSTEMS, [conn(10, 1, 2)], null);
		expect(edges).toEqual([
			{ a: 31000001, b: 31000002, via: 'wormhole', mass: 'stable', time: 'stable' },
		]);
	});

	it('leaves out ghosts, stargates and self-edges', () => {
		expect(chainEdges(SYSTEMS, [conn(10, 1, 3)], null)).toEqual([]);
		expect(chainEdges(SYSTEMS, [conn(10, 1, 2, 'stargate')], null)).toEqual([]);
		expect(chainEdges([...SYSTEMS, system(4, 31000001)], [conn(10, 1, 4)], null)).toEqual([]);
	});

	it('adds EVE Scout holes only when handed them', () => {
		const scout = [
			{
				hub_solar_system_id: 31000005,
				solar_system_id: 30000142,
				mass_status: 'reduced',
				time_status: 'eol',
			} as EveScoutConnection,
		];
		expect(chainEdges(SYSTEMS, [], null)).toEqual([]);
		expect(chainEdges(SYSTEMS, [], scout)).toEqual([
			{ a: 31000005, b: 30000142, via: 'evescout', mass: 'reduced', time: 'eol' },
		]);
	});
});

describe('routeConnectionIds', () => {
	const systems = [system(1, 100), system(2, 200), system(3, 300)];
	const connections = [conn(10, 1, 2), conn(11, 2, 3), conn(12, 1, 3)];

	it('is empty until the path has at least one hop', () => {
		expect(routeConnectionIds([], systems, connections).size).toBe(0);
		expect(routeConnectionIds([100], systems, connections).size).toBe(0);
	});

	it('picks the connections whose endpoints sit at adjacent path indices', () => {
		const ids = routeConnectionIds([100, 200, 300], systems, connections);
		expect(ids).toEqual(new Set([10, 11]));
	});

	it('skips a connection that shortcuts across the path', () => {
		expect(routeConnectionIds([100, 200, 300], systems, connections).has(12)).toBe(false);
	});
});
