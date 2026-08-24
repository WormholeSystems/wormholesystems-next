// The slice of the map the shared system menus read, declared here so `$lib` never
// imports from `routes/`. The map page's `MapState` satisfies it structurally: each
// member is one of its domain namespaces, narrowed to what the menus use.

import { getContext, setContext } from 'svelte';

import type { CharacterRef } from '$lib/api/types/CharacterRef';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { WatchlistEntry } from '$lib/api/types/WatchlistEntry';

export interface MapContext {
	mapId: number;
	readonly canWrite: boolean;
	systems: {
		readonly all: MapSystemView[];
		add(solarSystemId: number): void;
		setRally(mapSolarSystemId: number, value: boolean): void;
	};
	watchlist: {
		readonly all: WatchlistEntry[];
		add(solarSystemId: number): void;
	};
	characters: { readonly online: CharacterRef[] };
	waypoints: {
		set(destinationId: number, characterId: number, clearOthers: boolean): void;
		setAll(destinationId: number, clearOthers: boolean): void;
	};
	route: { fromId: number | null; toId: number | null };
}

const KEY = 'map-state';

/** Installed by the map screen; the getter keeps consumers on the live instance. */
export function setMapContext(get: () => MapContext): void {
	setContext(KEY, get);
}

/** The map behind the page, when there is one: these menus also render off the map. */
export function getMapContext(): (() => MapContext) | undefined {
	return getContext<(() => MapContext) | undefined>(KEY);
}
