import { error, redirect } from '@sveltejs/kit';

import { mapView } from '$lib/server/api';
import type { LayoutServerLoad } from './$types';

// Settings are for the people who run the map, never for a watcher following a link.
//
// The map is loaded once for the whole section. The per-viewer settings are not: they hang
// off their own key on the pages that use them, so a toggle does not refetch the chain.
export const load: LayoutServerLoad = async (event) => {
	const { me } = await event.parent();
	if (!me) redirect(302, '/login');

	event.depends('vector:map');
	const view = await mapView(event, Number(event.params.id)).catch(() => null);
	if (!view) error(404, 'That map is not one you can open.');
	return { view };
};
