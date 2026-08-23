import { describe, expect, it } from 'vitest';

import { findMatcher, type FindIndexes } from './find-conditions';

const indexes = (over: Partial<FindIndexes> = {}): FindIndexes => ({
	jove: new Set([1]),
	stations: new Set([2]),
	security: new Map([
		[10, 0.9],
		[11, 0.5],
		[12, 0.4],
		[13, 0.1],
		[14, 0.0],
		[15, -0.3],
	]),
	services: [{ id: 7, systems: new Set([3]) }],
	corporations: [{ id: 9, systems: new Set([4]) }],
	...over,
});

describe('findMatcher', () => {
	it('matches Jove observatories and NPC stations off their indexes', () => {
		expect(findMatcher('observatories', indexes())(1)).toBe(true);
		expect(findMatcher('observatories', indexes())(2)).toBe(false);
		expect(findMatcher('npc_stations', indexes())(2)).toBe(true);
		expect(findMatcher('npc_stations', indexes())(1)).toBe(false);
	});

	it('splits security at the in-game boundaries', () => {
		const at = (condition: string, id: number) => findMatcher(condition, indexes())(id);
		expect(at('highsec', 10)).toBe(true);
		expect(at('highsec', 11)).toBe(true);
		expect(at('highsec', 12)).toBe(false);
		expect(at('lowsec', 12)).toBe(true);
		expect(at('lowsec', 13)).toBe(true);
		expect(at('lowsec', 14)).toBe(false);
		expect(at('nullsec', 14)).toBe(true);
		expect(at('nullsec', 15)).toBe(true);
		expect(at('nullsec', 13)).toBe(false);
	});

	it('treats a system missing from the security table as nullsec', () => {
		expect(findMatcher('nullsec', indexes())(999)).toBe(true);
		expect(findMatcher('highsec', indexes())(999)).toBe(false);
	});

	it('matches station groups by their prefixed condition', () => {
		expect(findMatcher('service_7', indexes())(3)).toBe(true);
		expect(findMatcher('service_7', indexes())(4)).toBe(false);
		expect(findMatcher('corp_9', indexes())(4)).toBe(true);
		expect(findMatcher('corp_9', indexes())(3)).toBe(false);
	});

	it('matches nothing for a condition it does not know', () => {
		expect(findMatcher('service_404', indexes())(3)).toBe(false);
		expect(findMatcher('gibberish', indexes())(1)).toBe(false);
	});
});
