// One route search from the shared origin covering every row of a panel, rather than one
// per row. Component init only.

import { findRoutes, type RouteResult } from '$lib/routing/algorithm';
import type { MapState } from './map-state.svelte';

export function routeBatch(
	map: MapState,
	targets: () => number[],
	opts: { extraIgnored?: () => number[] } = {},
) {
	const routes = $derived.by<Map<number, RouteResult>>(() => {
		const graph = map.route.graph;
		const origin = map.routeOrigin;
		const wanted = [...new Set(targets())];
		if (!graph || origin === null || wanted.length === 0) return new Map();
		const extra = opts.extraIgnored?.() ?? [];
		const ignored = extra.length
			? new Set([...map.route.ignoredSystems, ...extra])
			: map.route.ignoredSystems;
		return findRoutes(graph, origin, wanted, map.routingSettings, ignored);
	});
	return {
		get routes() {
			return routes;
		},
	};
}
