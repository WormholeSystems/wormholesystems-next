import { currentCharacter, pinnedMaps } from '$lib/server/api';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async (event) => {
	const me = await currentCharacter(event);
	// The top bar's shortcuts, resolved with the page so they are there on first paint
	// rather than appearing a moment later.
	return { me, pinned: me ? await pinnedMaps(event) : [] };
};
