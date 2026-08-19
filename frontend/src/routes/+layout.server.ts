import { currentCharacter, pinnedMaps, serverStatus } from '$lib/server/api';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async (event) => {
	const me = await currentCharacter(event);
	// Everything the top bar draws, resolved with the page so none of it appears a moment
	// later: the shortcuts, and the server state beside the clock.
	const [pinned, status] = await Promise.all([
		me ? pinnedMaps(event) : Promise.resolve([]),
		serverStatus(event)
	]);
	return { me, pinned, status };
};
