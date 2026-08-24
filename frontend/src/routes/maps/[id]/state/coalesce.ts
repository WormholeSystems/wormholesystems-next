import type { QueryKey } from '@tanstack/svelte-query';

/**
 * Burst coalescing for query invalidations. The server is chatty in bursts: pasting a
 * scan publishes a frame per system, connection and placement it touched, all within a
 * few milliseconds, and invalidating per frame would abort and restart the same fetch
 * over and over. One timer collects the keys a burst names, then each is flushed once.
 */
export function createCoalescer(flush: (keys: QueryKey[]) => void, delayMs: number) {
	const pending = new Map<string, QueryKey>();
	let timer: ReturnType<typeof setTimeout> | null = null;
	return {
		schedule(queryKey: QueryKey) {
			pending.set(JSON.stringify(queryKey), queryKey);
			timer ??= setTimeout(() => {
				timer = null;
				const keys = [...pending.values()];
				pending.clear();
				flush(keys);
			}, delayMs);
		},
	};
}
