import { referenceCounts } from '$lib/server/api';

// The landing page states what this install knows, so the numbers come from its own
// database rather than being written into the page.
export async function load(event) {
	return { reference: await referenceCounts(event) };
}
