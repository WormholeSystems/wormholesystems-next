// The `?system=` deep link: resolving it into a node once, then keeping it written.
// Only ever writes the param; clearing it would race the load-time restore, which
// reads `?system=` before the map data has arrived and an active system exists.

import type { MapSystemView } from '$lib/api/types/MapSystemView';
import { solarSystemId } from '$lib/map/system';

/** The node id a `?system=` param points at, or null when it resolves to nothing. */
export function deepLinkTarget(systems: MapSystemView[], param: string | null): number | null {
	const wanted = Number(param);
	if (!wanted) return null;
	return systems.find((s) => solarSystemId(s) === wanted)?.id ?? null;
}

/** A copy of `url` with `?system=` pointing at the active system, or null when it already does. */
export function systemParamUrl(url: URL, activeSolarSystemId: number): URL | null {
	if (url.searchParams.get('system') === String(activeSolarSystemId)) return null;
	const next = new URL(url);
	next.searchParams.set('system', String(activeSolarSystemId));
	return next;
}
