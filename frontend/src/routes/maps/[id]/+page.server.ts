import { mapUserSettings, mapView } from '$lib/server/api';
import type { PageServerLoad } from './$types';

// The graph and the viewer's arrangement, fetched with the page rather than after it, so
// the first frame the page paints is the map rather than a placeholder. `MapState` starts
// from both and takes over; a failure is left to the client, which knows how to say why.
export const load: PageServerLoad = async (event) => {
	const id = Number(event.params.id);
	const [view, settings] = await Promise.all([
		mapView(event, id).catch(() => null),
		mapUserSettings(event, id).catch(() => null),
	]);
	// A map link is the one people actually paste at each other, so it says which map.
	return { view, settings, seo: { title: view?.map.name } };
};
