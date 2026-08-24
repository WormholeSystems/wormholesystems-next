// The slice of the map the shared system menus read, declared here so `$lib` never
// imports from `routes/`. The map page's `MapState` satisfies it structurally.

import { getContext, setContext } from 'svelte';

import type { CharacterRef } from '$lib/api/types/CharacterRef';
import type { GridConfig } from '$lib/api/types/GridConfig';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { WatchlistEntry } from '$lib/api/types/WatchlistEntry';
import type { MapAction } from '$lib/map/actions';
import type { Vec2 } from '$lib/map/helpers';

export interface MapContext {
	mapId: number;
	readonly canWrite: boolean;
	readonly systems: MapSystemView[];
	readonly myCharacters: CharacterRef[];
	readonly watchlist: WatchlistEntry[];
	readonly grid: GridConfig;
	camera: {
		pan: Vec2;
		readonly zoom: number;
		viewportRect(): { left: number; top: number; width: number; height: number };
	};
	route: { fromId: number | null; toId: number | null };
	run(action: MapAction, promise: Promise<unknown>, detail?: string): void;
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
