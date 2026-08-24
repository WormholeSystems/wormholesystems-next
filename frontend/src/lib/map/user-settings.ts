import { api } from '$lib/api/client';
import { apiAction } from '$lib/api/mutations';
import { key } from '$lib/api/queries';
import type { UpdateMapUserSettings } from '$lib/api/types/UpdateMapUserSettings';

/**
 * A saver for the viewer's own settings on a settings page: save the patch, refetch the
 * settings. Component init only (it creates a mutation). The pages that own a `MapState`
 * use its `patchUserSettings` instead, which also holds the write against a slower read.
 */
export function userSettingsSaver(mapId: () => number) {
	const act = apiAction(() => [key.userSettings(mapId())]);
	return (patch: UpdateMapUserSettings) =>
		act.mutate(() => api.updateMapUserSettings(mapId(), patch));
}
