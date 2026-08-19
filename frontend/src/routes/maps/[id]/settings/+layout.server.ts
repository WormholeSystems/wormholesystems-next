import { redirect } from '@sveltejs/kit';

import type { LayoutServerLoad } from './$types';

// Settings are for the people who run the map, never for a watcher following a link.
export const load: LayoutServerLoad = async ({ parent }) => {
	const { me } = await parent();
	if (!me) redirect(302, '/login');
};
