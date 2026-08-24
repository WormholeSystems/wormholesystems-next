import { describe, expect, it, vi } from 'vitest';

import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
import { createSystemResolver } from './resolve-cache.svelte';

const row = (id: number): SystemSearchResult => ({ id, name: `J${id}` }) as SystemSearchResult;

describe('createSystemResolver', () => {
	it('fetches each id once and shares concurrent asks', async () => {
		const fetchRows = vi.fn(async (ids: number[]) => ids.map(row));
		const resolver = createSystemResolver(fetchRows);
		const [a, b] = await Promise.all([resolver.resolve(1), resolver.resolve(1)]);
		expect(a?.name).toBe('J1');
		expect(b?.name).toBe('J1');
		expect(fetchRows).toHaveBeenCalledOnce();
		await resolver.resolve(1);
		expect(fetchRows).toHaveBeenCalledOnce();
	});

	it('never fetches what was seeded', async () => {
		const fetchRows = vi.fn(async (ids: number[]) => ids.map(row));
		const resolver = createSystemResolver(fetchRows);
		resolver.seed([row(5)]);
		expect(await resolver.resolve(5)).toEqual(row(5));
		expect(fetchRows).not.toHaveBeenCalled();
	});

	it('clears the in-flight mark when a batch fails, so a retry can fetch again', async () => {
		const fetchRows = vi
			.fn<(ids: number[]) => Promise<SystemSearchResult[]>>()
			.mockRejectedValueOnce(new Error('down'))
			.mockImplementation(async (ids: number[]) => ids.map(row));
		const resolver = createSystemResolver(fetchRows);
		expect(await resolver.resolve(7)).toBeUndefined();
		expect(await resolver.resolve(7)).toEqual(row(7));
		expect(fetchRows).toHaveBeenCalledTimes(2);
	});
});
