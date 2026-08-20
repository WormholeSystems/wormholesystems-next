import { accessList } from '$lib/server/api';
import type { PageServerLoad } from './$types';

// The grant list is this page's alone; the map itself comes from the section's layout.
export const load: PageServerLoad = async (event) => {
	event.depends('ws:access');
	return { access: await accessList(event, Number(event.params.id)).catch(() => []) };
};
