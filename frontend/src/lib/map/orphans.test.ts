import { describe, expect, it } from 'vitest';

import { orphanedSystems } from './orphans';

const system = (id: number, anchor: 'pinned' | 'home' | null = null) => ({
	id,
	is_pinned: anchor === 'pinned',
	is_home: anchor === 'home',
});
const edge = (from_system: number, to_system: number) => ({ from_system, to_system });
const ids = (systems: { id: number }[]) => systems.map((s) => s.id);

describe('orphanedSystems', () => {
	it('finds nothing when the map has no anchor to measure from', () => {
		const systems = [system(1), system(2)];
		expect(orphanedSystems(systems, [])).toEqual([]);
	});

	it('keeps everything a chain of hops still reaches', () => {
		const systems = [system(1, 'home'), system(2), system(3)];
		expect(orphanedSystems(systems, [edge(1, 2), edge(2, 3)])).toEqual([]);
	});

	it('reports the branch left behind when its hole collapses', () => {
		const systems = [system(1, 'home'), system(2), system(3), system(4)];
		expect(ids(orphanedSystems(systems, [edge(1, 2), edge(3, 4)]))).toEqual([3, 4]);
	});

	it('reaches through a connection stored either way round', () => {
		const systems = [system(1, 'pinned'), system(2)];
		expect(orphanedSystems(systems, [edge(2, 1)])).toEqual([]);
	});

	it('terminates on a cycle rather than walking it forever', () => {
		const systems = [system(1, 'pinned'), system(2), system(3), system(9)];
		const ring = [edge(1, 2), edge(2, 3), edge(3, 1)];
		expect(ids(orphanedSystems(systems, ring))).toEqual([9]);
	});

	it('measures from every anchor, not just the first', () => {
		const systems = [system(1, 'home'), system(5, 'pinned'), system(6)];
		expect(orphanedSystems(systems, [edge(5, 6)])).toEqual([]);
	});
});
