import { describe, expect, it } from 'vitest';

import { formatIsk, iskTone, timeAgo } from './format';

const NOW = new Date('2026-08-17T15:00:00Z');
const ago = (ms: number) => new Date(NOW.getTime() - ms).toISOString();

describe('timeAgo', () => {
	it('hands over cleanly from one unit to the next', () => {
		expect(timeAgo(ago(30_000), NOW)).toBe('just now');
		expect(timeAgo(ago(59_000), NOW)).toBe('just now');
		expect(timeAgo(ago(60_000), NOW)).toBe('1m ago');
		expect(timeAgo(ago(47 * 60_000), NOW)).toBe('47m ago');
		expect(timeAgo(ago(60 * 60_000), NOW)).toBe('1h ago');
		expect(timeAgo(ago(23 * 3_600_000), NOW)).toBe('23h ago');
	});

	it('never reports more hours than there are in a day', () => {
		// The gap legacy leaves: with a `> 1 day` test, a 30-hour-old kill reads "30h ago".
		expect(timeAgo(ago(24 * 3_600_000), NOW)).toBe('1d ago');
		expect(timeAgo(ago(30 * 3_600_000), NOW)).toBe('1d ago');
		expect(timeAgo(ago(47 * 3_600_000), NOW)).toBe('1d ago');
		expect(timeAgo(ago(48 * 3_600_000), NOW)).toBe('2d ago');
	});
});

describe('formatIsk', () => {
	it('reads the way a killboard writes it', () => {
		expect(formatIsk(2_400_000)).toBe('2.4M');
		expect(formatIsk(340_000_000)).toBe('340M');
		expect(formatIsk(1_240_000_000)).toBe('1.2B');
		expect(formatIsk(82_000_000_000)).toBe('82B');
	});

	it('says nothing rather than zero when the value is unknown', () => {
		expect(formatIsk(null)).toBeNull();
		expect(formatIsk(undefined)).toBeNull();
	});
});

describe('iskTone', () => {
	it('gets louder as the loss gets worse', () => {
		expect(iskTone(2_400_000)).toContain('muted');
		expect(iskTone(1_500_000_000)).toContain('amber');
		expect(iskTone(82_000_000_000)).toContain('red');
	});

	it('is quiet at the thresholds themselves, not before', () => {
		expect(iskTone(999_999_999)).toContain('muted');
		expect(iskTone(1_000_000_000)).toContain('amber');
		expect(iskTone(9_999_999_999)).toContain('amber');
		expect(iskTone(10_000_000_000)).toContain('red');
	});
});
