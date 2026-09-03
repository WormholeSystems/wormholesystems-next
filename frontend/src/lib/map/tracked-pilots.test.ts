import { describe, expect, it } from 'vitest';

import type { CharacterRef } from '$lib/api/types/CharacterRef';

import { trackedPilots } from './tracked-pilots';

const pilot = (id: number, active = false) =>
	({ character_id: id, name: `Pilot ${id}`, is_active: active }) as CharacterRef;

describe('trackedPilots', () => {
	it('falls back to the acting pilot when nothing is chosen', () => {
		expect(trackedPilots([pilot(1), pilot(2, true)], []).map((p) => p.character_id)).toEqual([2]);
	});

	it('takes the chosen set whether or not it holds the acting pilot', () => {
		const pilots = [pilot(1), pilot(2, true), pilot(3)];
		expect(trackedPilots(pilots, [1, 3]).map((p) => p.character_id)).toEqual([1, 3]);
	});

	it("ignores ids that are not this user's pilots any more", () => {
		expect(trackedPilots([pilot(1)], [1, 99]).map((p) => p.character_id)).toEqual([1]);
	});
});
