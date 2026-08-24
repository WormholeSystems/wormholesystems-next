// What the system menus do to the map, over the same narrow slice both chromes hold.

import { api } from '$lib/api/client';
import type { SystemStatus } from '$lib/api/types/SystemStatus';
import type { MapContext } from '$lib/components/system-menu/context';
import { centerWorld, freePosition } from './helpers';

/** Drops the system at a free spot near the middle of the current viewport. */
export function addToMap(map: MapContext, solarSystemId: number): void {
	const base = centerWorld(map.camera.pan, map.camera.zoom, map.camera.viewportRect());
	const spot = freePosition(map.systems, base, map.grid);
	map.run(
		'addSystem',
		api.addSystem({
			map_id: map.mapId,
			solar_system_id: solarSystemId,
			x: spot.x,
			y: spot.y,
			alias: null,
		}),
	);
}

export function addToWatchlist(map: MapContext, solarSystemId: number): void {
	map.run('watch', api.addWatchlistEntry({ map_id: map.mapId, solar_system_id: solarSystemId }));
}

export function setStatus(map: MapContext, mapSolarSystemId: number, status: SystemStatus): void {
	map.run(
		'setStatus',
		api.setStatus({ map_id: map.mapId, map_solar_system_id: mapSolarSystemId, status }),
	);
}

export function setPinned(map: MapContext, mapSolarSystemId: number, value: boolean): void {
	map.run(
		'setPinned',
		api.setPinned({ map_id: map.mapId, map_solar_system_id: mapSolarSystemId, value }),
	);
}

export function setHome(map: MapContext, mapSolarSystemId: number, value: boolean): void {
	map.run(
		'setHome',
		api.setHome({ map_id: map.mapId, map_solar_system_id: mapSolarSystemId, value }),
	);
}

export function setRally(map: MapContext, mapSolarSystemId: number, value: boolean): void {
	map.run(
		'setRally',
		api.setRally({ map_id: map.mapId, map_solar_system_id: mapSolarSystemId, value }),
	);
}

export function removeSystems(map: MapContext, mapSolarSystemIds: number[]): void {
	map.run(
		'removeSystems',
		api.removeSystems({ map_id: map.mapId, map_solar_system_ids: mapSolarSystemIds }),
	);
}
