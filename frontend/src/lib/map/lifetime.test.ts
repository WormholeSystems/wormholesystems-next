import { describe, expect, it } from 'vitest';

import type { MapConnection } from '$lib/api/types/MapConnection';

import { formatRemaining, lifetimeDeadline } from './lifetime';

const T0 = Date.parse('2026-09-03T10:00:00Z');
const HOUR = 3_600_000;

function hole(over: Partial<MapConnection> = {}) {
	return {
		kind: 'wormhole' as const,
		time_status: null,
		time_status_updated_at: null,
		created_at: new Date(T0).toISOString(),
		lifetime_hours: 24,
		...over,
	};
}

describe('lifetimeDeadline', () => {
	it('runs four hours from an EOL mark and one from a critical one', () => {
		const marked = new Date(T0 + 2 * HOUR).toISOString();
		expect(lifetimeDeadline(hole({ time_status: 'eol', time_status_updated_at: marked }))).toEqual({
			at: T0 + 6 * HOUR,
			estimated: false,
		});
		expect(
			lifetimeDeadline(hole({ time_status: 'critical', time_status_updated_at: marked })),
		).toEqual({ at: T0 + 3 * HOUR, estimated: false });
	});

	it('estimates from the class lifetime before any mark', () => {
		expect(lifetimeDeadline(hole())).toEqual({ at: T0 + 24 * HOUR, estimated: true });
		expect(lifetimeDeadline(hole({ lifetime_hours: 48 }))).toEqual({
			at: T0 + 48 * HOUR,
			estimated: true,
		});
	});

	it('has nothing to say about stargates or holes of unknown lifetime', () => {
		expect(lifetimeDeadline(hole({ kind: 'stargate' }))).toBeNull();
		expect(lifetimeDeadline(hole({ lifetime_hours: null }))).toBeNull();
	});
});

describe('formatRemaining', () => {
	it('rounds up to the minute and never goes negative', () => {
		expect(formatRemaining(T0 + 2 * HOUR + 13 * 60_000 + 1, T0)).toBe('2h 14m');
		expect(formatRemaining(T0 + 45 * 60_000, T0)).toBe('45m');
		expect(formatRemaining(T0 - HOUR, T0)).toBe('0m');
	});
});
