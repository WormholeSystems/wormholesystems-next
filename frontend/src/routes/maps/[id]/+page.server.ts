import { mapView } from '$lib/server/api';
import type { PageServerLoad } from './$types';

// The graph, fetched with the page rather than after it, so the map's name is on screen in
// the first frame instead of appearing a moment later. `MapState` starts from this and takes
// over from there; a failure is left to the client, which knows how to say why.
export const load: PageServerLoad = async (event) => {
	const view = await mapView(event, Number(event.params.id)).catch(() => null);
	return { view };
};
