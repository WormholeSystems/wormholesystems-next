import type { CharacterRef } from '$lib/api/types/CharacterRef';

/**
 * One origin for watchlist/find distances: the route planner's From, else the active
 * system, else the tracked character's location, else any online character's.
 */
export function routeOrigin(
	fromId: number | null,
	activeSolarSystemId: number | null,
	myCharacters: CharacterRef[],
): number | null {
	return (
		fromId ??
		activeSolarSystemId ??
		myCharacters.find((c) => c.is_active && c.online)?.solar_system_id ??
		myCharacters.find((c) => c.online && c.solar_system_id !== null)?.solar_system_id ??
		null
	);
}
