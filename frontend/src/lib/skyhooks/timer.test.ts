import { describe, expect, it } from 'vitest';

import { CLOSING_SOON_MS, describe as describeTiming, formatDuration, formatWindow, timing } from './timer';
import type { Skyhook } from '$lib/api/types/Skyhook';

const NOW = new Date('2026-08-17T13:00:00Z');

/** A skyhook whose window opens at `from` and runs the usual two hours. */
function skyhook(from: string, hours = 2): Skyhook {
	const start = new Date(from);
	return {
		planet_id: 40000001,
		planet_name: 'M2GJ-X III',
		planet_kind: 'lava',
		solar_system_id: 30003681,
		system_name: 'M2GJ-X',
		region: 'Feythabolis',
		region_id: 10000056,
		constellation_id: 20000812,
		security_status: -0.2,
		vulnerable_from: start.toISOString(),
		vulnerable_until: new Date(start.getTime() + hours * 3_600_000).toISOString()
	};
}

describe('timing', () => {
	it('counts down to the open while the window is still ahead', () => {
		const t = timing(skyhook('2026-08-17T13:30:00Z'), NOW);
		expect(t.status).toBe('upcoming');
		expect(t.untilMs).toBe(30 * 60_000);
	});

	it('counts down to the close once the window is open', () => {
		const t = timing(skyhook('2026-08-17T12:00:00Z'), NOW);
		expect(t.status).toBe('open');
		// Opened an hour ago, runs two: an hour left.
		expect(t.untilMs).toBe(60 * 60_000);
	});

	it('turns urgent in the last fifteen minutes', () => {
		// Opened at 11:10, so it closes at 13:10: ten minutes left.
		const t = timing(skyhook('2026-08-17T11:10:00Z'), NOW);
		expect(t.status).toBe('closing');
		expect(t.untilMs).toBeLessThan(CLOSING_SOON_MS);
	});

	it('is exactly on the boundary at fifteen minutes, not before', () => {
		// Closes at 13:15 on the dot.
		const t = timing(skyhook('2026-08-17T11:15:00Z'), NOW);
		expect(t.status).toBe('open');
	});

	it('reports a finished window as closed, with how long ago', () => {
		const t = timing(skyhook('2026-08-17T09:00:00Z'), NOW);
		expect(t.status).toBe('closed');
		expect(t.untilMs).toBe(2 * 60 * 60_000);
	});

	it('treats the opening instant as open rather than upcoming', () => {
		const t = timing(skyhook('2026-08-17T13:00:00Z'), NOW);
		expect(t.status).toBe('open');
	});
});

describe('formatDuration', () => {
	it('gets coarser as the number gets bigger', () => {
		expect(formatDuration(30_000)).toBe('<1m');
		expect(formatDuration(47 * 60_000)).toBe('47m');
		expect(formatDuration(2 * 3_600_000 + 5 * 60_000)).toBe('2h 05m');
		expect(formatDuration(3 * 3_600_000)).toBe('3h');
		expect(formatDuration(28 * 3_600_000)).toBe('1d 04h');
		expect(formatDuration(48 * 3_600_000)).toBe('2d');
	});

	it('reads the same whether the moment is ahead or behind', () => {
		expect(formatDuration(-47 * 60_000)).toBe('47m');
	});
});

describe('formatWindow', () => {
	it('writes the window in EVE time, not the browser’s', () => {
		expect(formatWindow(skyhook('2026-08-17T12:46:00Z'))).toBe('12:46 – 14:46 UTC');
	});
});

describe('describe', () => {
	it('says what the number means', () => {
		expect(describeTiming(timing(skyhook('2026-08-17T13:30:00Z'), NOW))).toBe('Raidable in 30m');
		expect(describeTiming(timing(skyhook('2026-08-17T12:00:00Z'), NOW))).toBe('Raidable for 1h');
		expect(describeTiming(timing(skyhook('2026-08-17T09:00:00Z'), NOW))).toBe('Closed 2h ago');
	});
});
