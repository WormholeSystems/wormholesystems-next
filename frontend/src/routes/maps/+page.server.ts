import { redirect } from '@sveltejs/kit';

import type { PageServerLoad } from './$types';

// The map list is yours, so it needs an account. A single map is not gated here: it may
// have been opened up to watchers, and the API decides that per request.
export const load: PageServerLoad = async ({ parent }) => {
	const { me } = await parent();
	if (!me) redirect(302, '/login');
};
