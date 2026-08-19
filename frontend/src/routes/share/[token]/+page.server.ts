import { error, redirect } from '@sveltejs/kit';

import { sharedMapId } from '$lib/server/api';
import type { PageServerLoad } from './$types';

/** How long a followed share link keeps working without the token in the address. */
const REMEMBER_FOR = 60 * 60 * 24 * 30;

// A share link is a way in, not a place: it hands the token over and drops you on the map
// itself, which is the same page everyone else is looking at. The token is kept in a cookie
// so the address bar goes back to being an ordinary map link, and so a bookmark of that
// link keeps working.
export const load: PageServerLoad = async (event) => {
	const mapId = await sharedMapId(event, event.params.token);
	if (!mapId) error(404, 'That share link does not lead anywhere any more.');

	event.cookies.set(`map_share_${mapId}`, event.params.token, {
		path: '/',
		httpOnly: true,
		sameSite: 'lax',
		maxAge: REMEMBER_FOR
	});
	redirect(302, `/maps/${mapId}`);
};
