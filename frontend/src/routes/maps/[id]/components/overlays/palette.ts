// Partitioning the palette's search hits: threat intel gets its own sections, and named
// systems split by whether they are already on the map.

import type { MapSearchHit } from '$lib/api/types/MapSearchHit';

/** Matched by what the system is called, not by intel typed onto it. */
export function byName(hit: MapSearchHit): boolean {
	return hit.matched === 'name' || hit.matched === 'alias';
}

export interface ThreatGroup {
	id: number;
	name: string;
	kind: string;
	total: number;
	hits: MapSearchHit[];
}

export interface PartitionedHits {
	/** One section per organisation, so the name is not repeated on every row. */
	threatGroups: ThreatGroup[];
	onMap: MapSearchHit[];
	offMap: MapSearchHit[];
	/** Occupier and notes matches, below every system named like the query. */
	intel: MapSearchHit[];
}

export function partitionHits(hits: MapSearchHit[]): PartitionedHits {
	const groups = new Map<number, ThreatGroup>();
	for (const hit of hits) {
		if (!hit.threat) continue;
		const t = hit.threat;
		const group = groups.get(t.entity_id) ?? {
			id: t.entity_id,
			name: t.name,
			kind: t.entity_type,
			total: 0,
			hits: [],
		};
		group.total += t.kills;
		group.hits.push(hit);
		groups.set(t.entity_id, group);
	}
	const named = hits.filter((h) => !h.threat);
	return {
		threatGroups: [...groups.values()],
		onMap: named.filter((h) => h.map_solar_system_id !== null && byName(h)),
		offMap: named.filter((h) => h.map_solar_system_id === null),
		intel: named.filter((h) => h.map_solar_system_id !== null && !byName(h)),
	};
}

/** Why this row matched, when it was not the name. */
export function matchHint(hit: MapSearchHit): string | null {
	if (hit.note_excerpt) return hit.note_excerpt;
	if (hit.matched === 'alias') return hit.alias;
	if (hit.matched === 'occupier') return hit.occupying_group;
	return null;
}
