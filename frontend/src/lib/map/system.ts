import type { MapSystemView } from '$lib/api/types/MapSystemView';

/**
 * The two halves of a map node, named, so a function can say which one it takes instead of
 * accepting either and checking again inside. Narrowed out of the wire union rather than
 * restated, so they cannot drift from what the server sends.
 */
export type MappedSystem = Extract<MapSystemView, { kind: 'system' }>;

export type GhostSystem = Extract<MapSystemView, { kind: 'ghost' }>;

/** For the callers that only want the id and do not care which half they were handed. */
export function solarSystemId(node: MapSystemView): number | null {
	return node.kind === 'system' ? node.solar_system_id : null;
}

export function systemName(node: MapSystemView): string | null {
	return node.kind === 'system' ? node.name : null;
}
