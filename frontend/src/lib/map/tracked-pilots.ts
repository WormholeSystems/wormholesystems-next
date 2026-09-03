// Which of a user's pilots build a map. A chosen set says so outright; none chosen means
// the pilot the session acts as, which is what every map did before there was a choice.

import type { CharacterRef } from '$lib/api/types/CharacterRef';

export function trackedPilots(pilots: CharacterRef[], trackedIds: number[]): CharacterRef[] {
	if (trackedIds.length === 0) return pilots.filter((p) => p.is_active);
	return pilots.filter((p) => trackedIds.includes(p.character_id));
}
