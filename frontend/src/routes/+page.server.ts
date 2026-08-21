import { referenceCounts } from '$lib/server/api';

// The landing page states what this install knows, so the numbers come from its own
// database rather than being written into the page.
export async function load(event) {
	return {
		reference: await referenceCounts(event),
		seo: {
			description:
				"Wormhole mapping and tracking for EVE Online. One live chain map for your corp, open source and self-hosted: signatures, connection mass and lifetime, and everyone's position from ESI.",
		},
	};
}
