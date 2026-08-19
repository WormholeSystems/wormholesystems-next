// Server-side API access for load functions: talks to the Axum backend directly (it is a
// separate service, so relative paths don't work here) and forwards the session cookie.

import { env } from '$env/dynamic/private';
import type { RequestEvent } from '@sveltejs/kit';

import type { CharacterSummary } from '$lib/api/types/CharacterSummary';
import type { MapEntry } from '$lib/api/types/MapEntry';

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

/**
 * The maps this user keeps in the top bar. A failure here is not worth a broken page: the
 * shortcuts are a convenience, and the rest of the app still works without them.
 */
export async function pinnedMaps(event: RequestEvent): Promise<MapEntry[]> {
	try {
		const maps = await get<MapEntry[]>(event, '/api/maps');
		return maps.filter((m) => m.is_pinned && !m.is_archived);
	} catch {
		return [];
	}
}
