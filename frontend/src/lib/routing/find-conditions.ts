// What the Find panel can search for, turned into a predicate over solar system ids.
// Station groups (services, owners) are matched by the panel itself, which intersects
// them at the station level; only the static conditions live here.

import { ccpRoundSecurity } from '$lib/security';

export interface FindIndexes {
	jove: ReadonlySet<number>;
	stations: ReadonlySet<number>;
	/** Security by solar system id; an unknown system counts as nullsec. */
	security: ReadonlyMap<number, number>;
}

/** The predicate for a Find condition. An unrecognised condition matches nothing. */
export function findMatcher(condition: string, indexes: FindIndexes): (id: number) => boolean {
	const sec = (id: number) => ccpRoundSecurity(indexes.security.get(id) ?? 0);
	const matchers: Record<string, (id: number) => boolean> = {
		observatories: (id) => indexes.jove.has(id),
		station: (id) => indexes.stations.has(id),
		highsec: (id) => sec(id) >= 0.5,
		lowsec: (id) => sec(id) >= 0.1 && sec(id) <= 0.4,
		nullsec: (id) => sec(id) <= 0,
	};
	return matchers[condition] ?? (() => false);
}
