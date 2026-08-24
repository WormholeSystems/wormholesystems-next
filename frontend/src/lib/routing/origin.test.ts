import { describe, expect, it } from 'vitest';

import type { CharacterRef } from '$lib/api/types/CharacterRef';
import { routeOrigin } from './origin';

const pilot = (over: Partial<CharacterRef>): CharacterRef =>
	({ is_active: false, online: false, solar_system_id: null, ...over }) as CharacterRef;

describe('routeOrigin', () => {
	const active = pilot({ is_active: true, online: true, solar_system_id: 100 });
	const alt = pilot({ online: true, solar_system_id: 200 });

	it('prefers the planner origin over everything', () => {
		expect(routeOrigin(1, 2, [active])).toBe(1);
	});

	it('falls back to the active system, then the tracked character', () => {
		expect(routeOrigin(null, 2, [active])).toBe(2);
		expect(routeOrigin(null, null, [active, alt])).toBe(100);
	});

	it('takes any online character with a location when the active one has none', () => {
		const activeNowhere = pilot({ is_active: true, online: true, solar_system_id: null });
		expect(routeOrigin(null, null, [activeNowhere, alt])).toBe(200);
	});

	it('is nothing with no origin at all', () => {
		expect(routeOrigin(null, null, [pilot({})])).toBeNull();
	});
});
