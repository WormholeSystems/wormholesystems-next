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
		[16, 0.4552], // displays as 0.5, so highsec
		[17, 0.02], // displays as 0.1, so lowsec
	]),
	...over,
});

describe('findMatcher', () => {
	it('matches Jove observatories and NPC stations off their indexes', () => {
		expect(findMatcher('observatories', indexes())(1)).toBe(true);
		expect(findMatcher('observatories', indexes())(2)).toBe(false);
		expect(findMatcher('station', indexes())(2)).toBe(true);
		expect(findMatcher('station', indexes())(1)).toBe(false);
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

	it('bands the displayed security, not the raw value', () => {
		const at = (condition: string, id: number) => findMatcher(condition, indexes())(id);
		expect(at('highsec', 16)).toBe(true);
		expect(at('lowsec', 16)).toBe(false);
		expect(at('lowsec', 17)).toBe(true);
		expect(at('nullsec', 17)).toBe(false);
	});

	it('treats a system missing from the security table as nullsec', () => {
		expect(findMatcher('nullsec', indexes())(999)).toBe(true);
		expect(findMatcher('highsec', indexes())(999)).toBe(false);
	});

	it('matches nothing for a condition it does not know', () => {
		expect(findMatcher('gibberish', indexes())(1)).toBe(false);
	});
});
