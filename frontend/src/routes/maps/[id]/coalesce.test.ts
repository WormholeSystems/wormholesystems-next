import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { createCoalescer } from './coalesce';

describe('createCoalescer', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('collapses a burst into one flush, deduplicated by key', () => {
		const flushed: unknown[][] = [];
		const c = createCoalescer((keys) => flushed.push(keys), 60);
		c.schedule(['maps', 1, 'view']);
		c.schedule(['maps', 1, 'signatures']);
		c.schedule(['maps', 1, 'view']);
		expect(flushed).toEqual([]);
		vi.advanceTimersByTime(60);
		expect(flushed).toEqual([
			[
				['maps', 1, 'view'],
				['maps', 1, 'signatures'],
			],
		]);
	});

	it('starts a fresh window after a flush', () => {
		const flushed: unknown[][] = [];
		const c = createCoalescer((keys) => flushed.push(keys), 60);
		c.schedule(['a']);
		vi.advanceTimersByTime(60);
		c.schedule(['b']);
		vi.advanceTimersByTime(60);
		expect(flushed).toEqual([[['a']], [['b']]]);
	});

	it('does not extend the window while a burst keeps arriving', () => {
		const flushed: unknown[][] = [];
		const c = createCoalescer((keys) => flushed.push(keys), 60);
		c.schedule(['a']);
		vi.advanceTimersByTime(40);
		c.schedule(['b']);
		vi.advanceTimersByTime(20);
		expect(flushed).toEqual([[['a'], ['b']]]);
	});
});
