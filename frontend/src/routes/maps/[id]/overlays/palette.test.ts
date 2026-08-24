import { describe, expect, it } from 'vitest';

import type { MapSearchHit } from '$lib/api/types/MapSearchHit';
import { byName, matchHint, partitionHits } from './palette';

const hit = (over: Partial<MapSearchHit>): MapSearchHit =>
	({
		matched: 'name',
		map_solar_system_id: null,
		threat: null,
		note_excerpt: null,
		alias: null,
		occupying_group: null,
		...over,
	}) as MapSearchHit;

describe('byName', () => {
	it('counts name and alias matches, not intel matches', () => {
		expect(byName(hit({ matched: 'name' }))).toBe(true);
		expect(byName(hit({ matched: 'alias' }))).toBe(true);
		expect(byName(hit({ matched: 'occupier' }))).toBe(false);
		expect(byName(hit({ matched: 'notes' }))).toBe(false);
	});
});

describe('partitionHits', () => {
	it('splits named hits by placement, intel matches below them', () => {
		const { onMap, offMap, intel } = partitionHits([
			hit({ map_solar_system_id: 1 }),
			hit({}),
			hit({ map_solar_system_id: 2, matched: 'occupier' }),
		]);
		expect(onMap).toHaveLength(1);
		expect(offMap).toHaveLength(1);
		expect(intel).toHaveLength(1);
	});

	it('groups threat hits per organisation with summed kills', () => {
		const threat = (entityId: number, kills: number) =>
			hit({
				threat: { entity_id: entityId, entity_type: 'alliance', name: `A${entityId}`, kills },
			} as Partial<MapSearchHit>);
		const { threatGroups, onMap } = partitionHits([threat(1, 3), threat(1, 4), threat(2, 1)]);
		expect(threatGroups.map((g) => [g.id, g.total])).toEqual([
			[1, 7],
			[2, 1],
		]);
		expect(onMap).toHaveLength(0);
	});
});

describe('matchHint', () => {
	it('prefers the note excerpt, then the alias, then the occupier', () => {
		expect(matchHint(hit({ note_excerpt: 'stash here', matched: 'alias', alias: '1a' }))).toBe(
			'stash here',
		);
		expect(matchHint(hit({ matched: 'alias', alias: '1a' }))).toBe('1a');
		expect(matchHint(hit({ matched: 'occupier', occupying_group: 'HK' }))).toBe('HK');
		expect(matchHint(hit({}))).toBeNull();
	});
});
