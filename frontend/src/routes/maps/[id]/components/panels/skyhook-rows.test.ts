import { describe, expect, it } from 'vitest';

import type { Skyhook } from '$lib/api/types/Skyhook';
import type { RouteResult } from '$lib/routing/algorithm';
import {
	SKYHOOK_COMPARATORS,
	buildSkyhookRows,
	liveSkyhookRows,
	skyhookCounts,
	skyhookTiebreak,
	type SkyhookRow,
} from './skyhook-rows';

const row = (over: Partial<SkyhookRow>): SkyhookRow =>
	({
		skyhook: { planet_id: 1, planet_name: 'I', region: 'R', planet_kind: 'lava' } as Skyhook,
		status: 'open',
		untilMs: 0,
		route: undefined,
		jumps: null,
		...over,
	}) as SkyhookRow;

describe('buildSkyhookRows', () => {
	it('pairs each skyhook with its window and route', () => {
		const skyhook = {
			solar_system_id: 100,
			planet_kind: 'lava',
			// A window opening in an hour, so the row reads as upcoming.
			vulnerable_from: new Date(Date.now() + 3_600_000).toISOString(),
			vulnerable_until: new Date(Date.now() + 2 * 3_600_000).toISOString(),
		} as unknown as Skyhook;
		const routes = new Map<number, RouteResult>([
			[100, { jumps: 4, route: [] } as unknown as RouteResult],
		]);
		const rows = buildSkyhookRows([skyhook], new Date(), routes);
		expect(rows[0].jumps).toBe(4);
		expect(rows[0].status).toBe('upcoming');
	});
});

describe('liveSkyhookRows and skyhookCounts', () => {
	it('drops closed windows and whatever the filter hides, counting per reagent', () => {
		const rows = [
			row({ status: 'open' }),
			row({ status: 'closed' }),
			row({ status: 'upcoming', skyhook: { planet_id: 2, planet_kind: 'ice' } as Skyhook }),
		];
		const live = liveSkyhookRows(rows, ['open']);
		expect(live).toHaveLength(1);
		expect(skyhookCounts(liveSkyhookRows(rows, ['open', 'upcoming']))).toEqual({
			lava: 1,
			ice: 1,
			other: 0,
		});
	});
});

describe('orderings', () => {
	it('sorts unreachable rows last in the jumps column', () => {
		expect(SKYHOOK_COMPARATORS.jumps(row({ jumps: 3 }), row({ jumps: null }))).toBeLessThan(0);
		expect(SKYHOOK_COMPARATORS.jumps(row({ jumps: null }), row({ jumps: 3 }))).toBeGreaterThan(0);
	});

	it('puts open windows before upcoming ones, then the sooner moment', () => {
		expect(
			SKYHOOK_COMPARATORS.timer(row({ status: 'open' }), row({ status: 'upcoming' })),
		).toBeLessThan(0);
		expect(SKYHOOK_COMPARATORS.timer(row({ untilMs: 100 }), row({ untilMs: 500 }))).toBeLessThan(0);
	});

	it('breaks ties by planet, so ticking timers never reorder equals', () => {
		const a = row({ skyhook: { planet_id: 1 } as Skyhook });
		const b = row({ skyhook: { planet_id: 2 } as Skyhook });
		expect(skyhookTiebreak(a, b)).toBeLessThan(0);
	});
});
