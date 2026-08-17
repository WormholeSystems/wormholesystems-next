// How the pilot list is ordered.
//
// Not alphabetical: the question the list answers is "who could do something about this",
// so pilots who cannot sink to the bottom. Nothing is hidden, because a docked alt three
// jumps out is still worth knowing about, it just is not the first thing you need to read.

import type { MapCharacter } from '$lib/api/types/MapCharacter';

/** Covert Ops frigates: almost always mid-scan rather than available. */
export const COVERT_OPS_GROUP = 830;

/** A pod is a pilot who has already lost the fight, or has not undocked into anything. */
export function isPod(pilot: MapCharacter): boolean {
	return pilot.ship_type === 'Capsule';
}

export function isScanner(pilot: MapCharacter): boolean {
	return pilot.ship_group_id === COVERT_OPS_GROUP;
}

/** Shown dimmed: present on the map, but not on grid in anything that matters. */
export function isIdle(pilot: MapCharacter): boolean {
	return pilot.is_docked || isPod(pilot);
}

/**
 * The de-prioritisation cascade, worst-last: pods, then docked pilots, then scanners, then
 * by name. Ported from legacy, which got this right.
 */
export function orderPilots(pilots: MapCharacter[]): MapCharacter[] {
	return [...pilots].sort((a, b) => {
		if (isPod(a) !== isPod(b)) return isPod(a) ? 1 : -1;
		if (a.is_docked !== b.is_docked) return a.is_docked ? 1 : -1;
		if (isScanner(a) !== isScanner(b)) return isScanner(a) ? 1 : -1;
		return a.name.localeCompare(b.name);
	});
}
