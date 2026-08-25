import { describe, expect, it } from 'vitest';

import { ccpRoundSecurity } from './security';

describe('ccpRoundSecurity', () => {
	it('rounds to the displayed tenth', () => {
		expect(ccpRoundSecurity(0.9134)).toBe(0.9);
		expect(ccpRoundSecurity(0.4552)).toBe(0.5);
		expect(ccpRoundSecurity(0.4442)).toBe(0.4);
	});

	it('rounds a barely-positive sec up to lowsec, not down to zero', () => {
		expect(ccpRoundSecurity(0.02)).toBe(0.1);
		expect(ccpRoundSecurity(0.0001)).toBe(0.1);
	});

	it('leaves exact zero alone and rounds negatives away from zero', () => {
		expect(ccpRoundSecurity(0)).toBe(0);
		expect(ccpRoundSecurity(-0.014694)).toBe(-0);
		expect(ccpRoundSecurity(-0.05)).toBe(-0.1);
		expect(ccpRoundSecurity(-0.987)).toBe(-1);
	});
});
