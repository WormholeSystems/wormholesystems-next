// What the Find panel can search for, turned into a predicate over solar system ids. The
// static conditions and the per-station groups are the same question ("is this system one
// of those?"), so they share one table.

export interface StationGroupIndex {
	id: number;
	systems: ReadonlySet<number>;
}

export interface FindIndexes {
	jove: ReadonlySet<number>;
	stations: ReadonlySet<number>;
	/** Security by solar system id; an unknown system counts as nullsec. */
	security: ReadonlyMap<number, number>;
	services: StationGroupIndex[];
	corporations: StationGroupIndex[];
}

/** The predicate for a Find condition. An unrecognised condition matches nothing. */
export function findMatcher(condition: string, indexes: FindIndexes): (id: number) => boolean {
	const sec = (id: number) => indexes.security.get(id) ?? 0;
	const matchers: Record<string, (id: number) => boolean> = {
		observatories: (id) => indexes.jove.has(id),
		npc_stations: (id) => indexes.stations.has(id),
		highsec: (id) => sec(id) >= 0.5,
		lowsec: (id) => sec(id) >= 0.1 && sec(id) <= 0.4,
		nullsec: (id) => sec(id) <= 0,
	};
	for (const svc of indexes.services) {
		matchers[`service_${svc.id}`] = (id) => svc.systems.has(id);
	}
	for (const corp of indexes.corporations) {
		matchers[`corp_${corp.id}`] = (id) => corp.systems.has(id);
	}
	return matchers[condition] ?? (() => false);
}
