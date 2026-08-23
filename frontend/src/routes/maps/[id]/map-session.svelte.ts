// One visit's plumbing: the initial loads, both sockets, presence polling, and the focus
// listener that catches jumps taken while the tab was in the background. Returns the
// cleanup, so a single $effect can hand the whole lifecycle over.

import { solarSystemId } from '$lib/map/system';
import { openMapSocket, openUserSocket } from '$lib/ws';
import type { MapState } from './map-state.svelte';
import type { JumpTracker } from './tracking.svelte';

export function connectMapSession(
	map: MapState,
	tracker: JumpTracker,
	/** A `?system=` deep link to activate once the graph is in; 0 for none. */
	wantedSystem: number,
): (() => void) | undefined {
	if (map.mapId === 0) return;
	map.loadGrid();
	map.refetch().then(() => {
		if (wantedSystem && map.activeId === null) {
			map.activeId = map.systems.find((x) => solarSystemId(x) === wantedSystem)?.id ?? null;
		}
	});
	map.loadUserSettings();
	map.loadRoutingGraph();
	map.loadIgnored();
	// Below here is about the pilot at the keyboard: presence, jump tracking, the private
	// channel. A watcher has none of it.
	if (!map.signedIn) {
		const closeShared = openMapSocket(
			map.mapId,
			(event) => event && map.applyEvent(event),
			(state) => (map.socket = state),
		);
		return () => closeShared();
	}
	tracker.refresh();
	map.fetchCharacters();
	const observe = () => tracker.refresh();
	// Movement arrives over the sockets; this is only the net under a dropped frame.
	const presence = setInterval(() => {
		map.fetchCharacters();
		observe();
	}, 120_000);
	// The character's own status change is how a jump is normally noticed within seconds.
	const closeUserWs = openUserSocket((event) => {
		if (event.type === 'character_status_changed') observe();
	});
	// Flying happens in the game client, so a jump has usually already happened by the
	// time the tab is looked at again.
	window.addEventListener('focus', observe);
	const closeWs = openMapSocket(
		map.mapId,
		(event) => event && map.applyEvent(event),
		(state) => (map.socket = state),
	);
	return () => {
		clearInterval(presence);
		window.removeEventListener('focus', observe);
		closeUserWs();
		closeWs();
	};
}
