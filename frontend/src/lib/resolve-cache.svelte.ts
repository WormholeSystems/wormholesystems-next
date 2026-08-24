import { api } from '$lib/api/client';
import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';

/**
 * A grow-only, request-deduplicating cache over a batch resolver. Factory-shaped so a
 * test can construct a fresh one with a stub fetcher; the app shares the singleton below.
 */
export function createSystemResolver(fetchRows: (ids: number[]) => Promise<SystemSearchResult[]>) {
	let resolved = $state(new Map<number, SystemSearchResult>());
	// One promise per id currently on the wire, so concurrent asks share the fetch.
	const pending = new Map<number, Promise<void>>();

	async function fetchMissing(ids: number[]): Promise<void> {
		const waits: Promise<void>[] = [];
		const missing: number[] = [];
		for (const id of new Set(ids)) {
			if (resolved.has(id)) continue;
			const wait = pending.get(id);
			if (wait) waits.push(wait);
			else missing.push(id);
		}
		if (missing.length > 0) {
			const batch = fetchRows(missing)
				.then((rows) => {
					const next = new Map(resolved);
					for (const row of rows) next.set(row.id, row);
					resolved = next;
				})
				.catch(() => {})
				.finally(() => {
					for (const id of missing) pending.delete(id);
				});
			for (const id of missing) pending.set(id, batch);
			waits.push(batch);
		}
		await Promise.all(waits);
	}

	return {
		/** What has arrived (or been seeded) for `id`; undefined until then. */
		get(id: number): SystemSearchResult | undefined {
			return resolved.get(id);
		},
		/** Fire-and-forget: fetch whatever of `ids` is neither known nor already on the wire. */
		ensure(ids: number[]) {
			void fetchMissing(ids);
		},
		/** As `ensure`, awaitable for one id. Never throws; undefined when the fetch fails. */
		async resolve(id: number): Promise<SystemSearchResult | undefined> {
			await fetchMissing([id]);
			return resolved.get(id);
		},
		/** Hand over rows another payload already carried, so nobody fetches them again. */
		seed(rows: Iterable<SystemSearchResult>) {
			let next: Map<number, SystemSearchResult> | null = null;
			for (const row of rows) {
				if (resolved.has(row.id)) continue;
				(next ??= new Map(resolved)).set(row.id, row);
			}
			if (next) resolved = next;
		},
	};
}

/**
 * The one cache behind `api.resolveSystems`. Every surface that needs display data for a
 * solar system asks here, so a system goes over the wire at most once per session no
 * matter how many panels name it. The map page seeds it with its placed systems, which
 * therefore never hit the network at all.
 */
export const systemResolver = createSystemResolver((ids) => api.resolveSystems(ids));
