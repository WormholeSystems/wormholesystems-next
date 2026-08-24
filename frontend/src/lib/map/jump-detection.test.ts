import { describe, expect, it } from 'vitest';

import { detectJump } from './jump-detection';

describe('detectJump', () => {
	it('accepts the same character in two different systems', () => {
		expect(
			detectJump({ characterId: 9, systemId: 100 }, { characterId: 9, systemId: 200 }),
		).toEqual({ from: 100, to: 200 });
	});

	it('rejects the first reading of a session', () => {
		expect(
			detectJump({ characterId: null, systemId: null }, { characterId: 9, systemId: 100 }),
		).toBeNull();
	});

	it('rejects a logout, where the new reading has no system', () => {
		expect(
			detectJump({ characterId: 9, systemId: 100 }, { characterId: 9, systemId: null }),
		).toBeNull();
	});

	it('rejects standing still', () => {
		expect(
			detectJump({ characterId: 9, systemId: 100 }, { characterId: 9, systemId: 100 }),
		).toBeNull();
	});

	it('rejects a character switch, even across systems', () => {
		expect(
			detectJump({ characterId: 9, systemId: 100 }, { characterId: 7, systemId: 200 }),
		).toBeNull();
	});
});
