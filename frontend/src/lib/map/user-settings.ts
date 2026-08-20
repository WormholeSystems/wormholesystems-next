import { invalidate } from '$app/navigation';

import { api } from '$lib/api/client';
import type { UpdateMapUserSettings } from '$lib/api/types/UpdateMapUserSettings';

/**
 * Save one or more of the viewer's own settings from a settings page, then re-read them.
 * The pages that own a `MapState` use its `patchUserSettings` instead, which also holds the
 * write against a slower read.
 */
export function saveUserSettings(mapId: number, patch: UpdateMapUserSettings): Promise<void> {
	return api
		.updateMapUserSettings(mapId, patch)
		.then(() => invalidate('ws:user-settings'))
		.catch(() => {});
}
