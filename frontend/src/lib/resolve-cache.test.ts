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

	it('coalesces every ensure from one render pass into a single batch', async () => {
		const fetchRows = vi.fn(async (ids: number[]) => ids.map(row));
		const resolver = createSystemResolver(fetchRows);
		resolver.ensure([1]);
		resolver.ensure([2, 3]);
		resolver.ensure([1]);
		expect(fetchRows).not.toHaveBeenCalled();
		await Promise.resolve();
		expect(fetchRows).toHaveBeenCalledOnce();
		expect(fetchRows).toHaveBeenCalledWith([1, 2, 3]);
	});

	it('asks about an id the server does not know exactly once', async () => {
		// A successful-but-empty answer must not reassign the cache either: reactive
		// readers retrigger on every cache change, which would loop forever.
		const fetchRows = vi.fn(async (ids: number[]) => ids.filter((id) => id !== 404).map(row));
		const resolver = createSystemResolver(fetchRows);
		expect(await resolver.resolve(404)).toBeUndefined();
		expect(await resolver.resolve(404)).toBeUndefined();
		resolver.ensure([404]);
		await Promise.resolve();
		expect(fetchRows).toHaveBeenCalledOnce();
		expect(await resolver.resolve(1)).toEqual(row(1));
	});

	it('splits an oversized ask at the server cap', async () => {
		const fetchRows = vi.fn(async (ids: number[]) => ids.map(row));
		const resolver = createSystemResolver(fetchRows);
		const ids = Array.from({ length: 250 }, (_, i) => i + 1);
		resolver.ensure(ids);
		await Promise.resolve();
		expect(fetchRows).toHaveBeenCalledTimes(2);
		expect(fetchRows.mock.calls[0][0]).toHaveLength(200);
		expect(fetchRows.mock.calls[1][0]).toHaveLength(50);
		await resolver.resolve(250);
		expect(fetchRows).toHaveBeenCalledTimes(2);
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
