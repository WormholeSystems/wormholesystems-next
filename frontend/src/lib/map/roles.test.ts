import { describe, expect, it } from 'vitest';

import { atLeast, byRole } from './roles';

describe('roles', () => {
	it('orders the way the backend does', () => {
		expect(atLeast('owner', 'manager')).toBe(true);
		expect(atLeast('manager', 'manager')).toBe(true);
		expect(atLeast('member', 'manager')).toBe(false);
		expect(atLeast('viewer', 'member')).toBe(false);
	});

	it('treats no role as a viewer, which is what a shared map hands out', () => {
		expect(atLeast(null, 'member')).toBe(false);
		expect(atLeast(undefined, 'member')).toBe(false);
		expect(atLeast('viewer', 'viewer')).toBe(true);
	});

	it('sorts owners first', () => {
		expect(['viewer', 'owner', 'member', 'manager'].sort(byRole as never)).toEqual([
			'owner',
			'manager',
			'member',
			'viewer',
		]);
	});
});
