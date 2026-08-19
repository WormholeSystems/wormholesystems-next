import { error } from '@sveltejs/kit';

import { sharedMap } from '$lib/server/api';
import type { PageServerLoad } from './$types';

// Resolved on the server so a share link is a page, not an app that then asks for one:
// somebody following it may have no account, and should see the chain, not a spinner.
export const load: PageServerLoad = async (event) => {
	const shared = await sharedMap(event, event.params.token);
	if (!shared) error(404, 'That share link does not lead anywhere any more.');
	return shared;
};
