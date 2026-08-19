import { error, redirect } from '@sveltejs/kit';

import { mapUserSettings, mapView } from '$lib/server/api';
import type { LayoutServerLoad } from './$types';

// Settings are for the people who run the map, never for a watcher following a link.
//
// The map and the viewer's own settings are loaded once here for the whole section. Both
// carry a dependency key, so a page that changes one invalidates just that.
export const load: LayoutServerLoad = async (event) => {
	const { me } = await event.parent();
	if (!me) redirect(302, '/login');

	const mapId = Number(event.params.id);
	event.depends('vector:map', 'vector:user-settings');
	const [view, settings] = await Promise.all([
		mapView(event, mapId).catch(() => null),
		mapUserSettings(event, mapId).catch(() => null)
	]);
	if (!view) error(404, 'That map is not one you can open.');
	return { view, settings };
};
