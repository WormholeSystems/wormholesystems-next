// The maps list's pure half: filtering, ordering, and the header totals.

import type { MapEntry } from '$lib/api/types/MapEntry';

export function filterMaps(maps: MapEntry[], query: string): MapEntry[] {
	const q = query.trim().toLowerCase();
	if (!q) return maps;
	return maps.filter(
		(m) => m.name.toLowerCase().includes(q) || (m.description ?? '').toLowerCase().includes(q),
	);
}

/** Most recently touched first; a map nobody has changed yet falls back to its age. */
export function byRecency(a: MapEntry, b: MapEntry): number {
	const at = new Date(a.last_activity ?? a.created_at).getTime();
	const bt = new Date(b.last_activity ?? b.created_at).getTime();
	return bt - at;
}

export function splitArchived(maps: MapEntry[]): { active: MapEntry[]; archived: MapEntry[] } {
	return {
		active: maps.filter((m) => !m.is_archived).sort(byRecency),
		archived: maps.filter((m) => m.is_archived).sort(byRecency),
	};
}

export function totalsOf(active: MapEntry[]): { maps: number; systems: number; pilots: number } {
	return {
		maps: active.length,
		systems: active.reduce((n, m) => n + m.system_count, 0),
		pilots: active.reduce((n, m) => n + m.pilots_online, 0),
	};
}
