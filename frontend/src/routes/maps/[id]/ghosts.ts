// Putting the far side of a scanned wormhole on the map before anyone has flown it.
//
// Lives outside the signatures panel because every way a wormhole signature comes into
// being ends here: a pasted scan, a row typed in by hand, or an existing signature whose
// category is changed to Wormhole. Unlinking is deliberately not one of them — detaching
// a signature from a connection says "this is not that hole", and drawing a fresh node
// for it would argue back.

import { api } from '$lib/api/client';
import { freePosition } from '$lib/map/helpers';
import type { MapState } from './map-state.svelte';

/**
 * Give every wormhole scanned in `solarSystemId` that is not on the map yet a node
 * hanging off it, when the map is set up for it.
 *
 * Idempotent: a hole already on the map carries a connection, and those are skipped, so
 * this can run after any signature write without piling up duplicates.
 */
export async function ghostUnmappedHoles(map: MapState, solarSystemId: number | null) {
	if (!map.data?.map.ghost_unlinked_wormholes || solarSystemId === null) return;
	const from = map.systems.find((s) => s.solar_system_id === solarSystemId);
	if (!from) return;

	const fresh = await api.listSignatures(map.mapId);
	const unmapped = fresh.filter(
		(sig) =>
			sig.solar_system_id === solarSystemId &&
			sig.group === 'wormhole' &&
			sig.connection_id === null
	);

	// Each ghost has to dodge the ones just placed, which are not in `map.systems` yet.
	const taken = [...map.systems];
	for (const sig of unmapped) {
		const at = freePosition(taken, { x: from.position_x, y: from.position_y }, map.grid);
		taken.push({ ...from, id: -sig.id, position_x: at.x, position_y: at.y });
		await api.addGhostSystem({
			map_id: map.mapId,
			from_system: from.id,
			signature_pk: sig.id,
			x: at.x,
			y: at.y,
			size: sig.size ?? undefined
		});
	}
}
