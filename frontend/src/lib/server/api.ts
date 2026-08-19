// Server-side API access for load functions: talks to the Axum backend directly (it is a
// separate service, so relative paths don't work here) and forwards the session cookie.

import { env } from '$env/dynamic/private';
import type { RequestEvent } from '@sveltejs/kit';

import type { CharacterSummary } from '$lib/api/types/CharacterSummary';
import type { ServerStatus } from '$lib/api/types/ServerStatus';
import type { MapEntry } from '$lib/api/types/MapEntry';
import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
import type { MapView } from '$lib/api/types/MapView';
import type { AccessEntry } from '$lib/api/types/AccessEntry';

const base = () => env.API_BASE ?? 'http://127.0.0.1:3000';

async function get<T>(event: RequestEvent, path: string): Promise<T> {
	const res = await event.fetch(`${base()}${path}`, {
		headers: { cookie: event.request.headers.get('cookie') ?? '' }
	});
	if (!res.ok) throw new Error(`API ${path} failed: ${res.status}`);
	return res.json();
}

/** The signed-in character, or null. Used by the layout and the /maps auth gate. */
export function currentCharacter(event: RequestEvent): Promise<CharacterSummary | null> {
	return get<CharacterSummary | null>(event, '/api/me');
}

/** The maps this user keeps in the top bar. A failure is swallowed, they are a convenience. */
export async function pinnedMaps(event: RequestEvent): Promise<MapEntry[]> {
	try {
		const maps = await get<MapEntry[]>(event, '/api/maps');
		return maps.filter((m) => m.is_pinned && !m.is_archived);
	} catch {
		return [];
	}
}

/**
 * The map a share link leads to, or null. A withdrawn token and a mistyped one both come back
 * as not found, which is the point of a secret in a URL.
 */
export async function sharedMapId(event: RequestEvent, token: string): Promise<number | null> {
	try {
		const view = await get<MapView>(event, `/api/share/${encodeURIComponent(token)}`);
		return view.map.id;
	} catch {
		return null;
	}
}

/** A map's graph and the caller's role on it, for the settings pages. */
export function mapView(event: RequestEvent, mapId: number): Promise<MapView> {
	return get<MapView>(event, `/api/maps/${mapId}`);
}

/** Every map the caller can see. */
export function myMaps(event: RequestEvent): Promise<MapEntry[]> {
	return get<MapEntry[]>(event, '/api/maps');
}

/** The caller's own settings for one map. */
export function mapUserSettings(event: RequestEvent, mapId: number): Promise<MapUserSettings> {
	return get<MapUserSettings>(event, `/api/maps/${mapId}/settings/user`);
}

/** Who has been granted access to a map. */
export function accessList(event: RequestEvent, mapId: number): Promise<AccessEntry[]> {
	return get<AccessEntry[]>(event, `/api/maps/${mapId}/access`);
}

/**
 * Tranquility's state for the top bar. A failure here is not worth a broken page: the
 * component polls anyway, and an unknown state is one it already knows how to draw.
 */
export function serverStatus(event: RequestEvent): Promise<ServerStatus | null> {
	return get<ServerStatus>(event, '/api/server-status').catch(() => null);
}
