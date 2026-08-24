// Setting in-game waypoints from a menu. Four surfaces offer the same picker; the logic
// lives once, over the narrow slice of the map they all hold.

import { api } from '$lib/api/client';
import type { CharacterRef } from '$lib/api/types/CharacterRef';
import type { MapAction } from '$lib/map/actions';

interface WaypointHost {
	readonly myCharacters: CharacterRef[];
	run(action: MapAction, promise: Promise<unknown>, detail?: string): void;
}

/** Who can receive a waypoint right now. */
export function onlineCharacters(map: WaypointHost): CharacterRef[] {
	return map.myCharacters.filter((c) => c.online);
}

export function setWaypoint(
	map: WaypointHost,
	destinationId: number,
	characterId: number,
	clearOthers: boolean,
): void {
	map.run(
		'setWaypoint',
		api.setWaypoint({
			character_id: characterId,
			destination_id: destinationId,
			clear_other_waypoints: clearOthers,
		}),
	);
}

export function setWaypointAll(
	map: WaypointHost,
	destinationId: number,
	clearOthers: boolean,
): void {
	map.run(
		'setWaypoint',
		api.setWaypointAll({ destination_id: destinationId, clear_other_waypoints: clearOthers }),
	);
}
