import { currentCharacter, myMaps, serverStatus } from '$lib/server/api';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async (event) => {
	// Everything the top bar draws, resolved with the page so none of it appears a moment
	// later: the map list its shortcuts come from, and the server state beside the clock.
	// All three at once, since none of them needs an answer from the others; a signed-out
	// visitor gets an empty list rather than a wave spent finding out they are signed out.
	const [me, maps, status] = await Promise.all([
		currentCharacter(event),
		myMaps(event).catch(() => []),
		serverStatus(event),
	]);
	return { me, maps, status };
};
