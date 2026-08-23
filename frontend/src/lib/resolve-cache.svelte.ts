import { api } from '$lib/api/client';
import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';

/**
 * A grow-only cache over `api.resolveSystems`, for surfaces without a MapState to ask.
 * `ensure` fetches whatever is not yet known; `get` answers from what has arrived.
 */
export function resolveCache() {
	let resolved = $state<Map<number, SystemSearchResult>>(new Map());
	return {
		get(id: number): SystemSearchResult | undefined {
			return resolved.get(id);
		},
		ensure(ids: number[]) {
			const missing = [...new Set(ids.filter((id) => !resolved.has(id)))];
			if (missing.length === 0) return;
			api
				.resolveSystems(missing)
				.then((rows) => {
					const next = new Map(resolved);
					for (const row of rows) next.set(row.id, row);
					resolved = next;
				})
				.catch(() => {});
		},
	};
}
