import { redirect } from '@sveltejs/kit';

import type { LayoutServerLoad } from './$types';

// Auth gate for /maps and below; the API enforces auth independently per request.
export const load: LayoutServerLoad = async ({ parent }) => {
	const { me } = await parent();
	if (!me) redirect(302, '/login');
};
