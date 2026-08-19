/** The two shapes the rule needs; anything with these fields will do. */
interface Anchorable {
	id: number;
	is_pinned: boolean;
	is_home: boolean;
}

interface Edge {
	from_system: number;
	to_system: number;
}

/**
 * The dead branches: systems no anchor still reaches through the connections, where an
 * anchor is a pinned system or the home system.
 *
 * A map with no anchors at all has nothing orphaned rather than everything: without a place
 * to measure from, "unreachable" would mean the whole map.
 */
export function orphanedSystems<T extends Anchorable>(systems: T[], connections: Edge[]): T[] {
	const anchors = systems.filter((s) => s.is_pinned || s.is_home).map((s) => s.id);
	if (anchors.length === 0) return [];

	const neighbours = new Map<number, number[]>();
	const link = (from: number, to: number) => {
		const seen = neighbours.get(from);
		if (seen) seen.push(to);
		else neighbours.set(from, [to]);
	};
	for (const c of connections) {
		link(c.from_system, c.to_system);
		link(c.to_system, c.from_system);
	}

	const reachable = new Set(anchors);
	const queue = [...anchors];
	while (queue.length > 0) {
		for (const next of neighbours.get(queue.pop()!) ?? []) {
			if (reachable.has(next)) continue;
			reachable.add(next);
			queue.push(next);
		}
	}
	return systems.filter((s) => !reachable.has(s.id));
}
