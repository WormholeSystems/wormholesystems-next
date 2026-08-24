import { describe, expect, it } from 'vitest';

import { formatIsk, iskSeverity, timeAgo, timeAgoShort, utcShort } from './format';

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

describe('timeAgoShort', () => {
	it('is timeAgo at column width', () => {
		expect(timeAgoShort(ago(30_000), NOW)).toBe('now');
		expect(timeAgoShort(ago(60_000), NOW)).toBe('1m');
		expect(timeAgoShort(ago(47 * 60_000), NOW)).toBe('47m');
		expect(timeAgoShort(ago(60 * 60_000), NOW)).toBe('1h');
		expect(timeAgoShort(ago(23 * 3_600_000), NOW)).toBe('23h');
		expect(timeAgoShort(ago(30 * 3_600_000), NOW)).toBe('1d');
		expect(timeAgoShort(ago(48 * 3_600_000), NOW)).toBe('2d');
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

describe('iskSeverity', () => {
	it('gets louder as the loss gets worse', () => {
		expect(iskSeverity(2_400_000)).toBe('routine');
		expect(iskSeverity(1_500_000_000)).toBe('notable');
		expect(iskSeverity(82_000_000_000)).toBe('severe');
		expect(iskSeverity(null)).toBe('unknown');
	});

	it('is quiet at the thresholds themselves, not before', () => {
		expect(iskSeverity(999_999_999)).toBe('routine');
		expect(iskSeverity(1_000_000_000)).toBe('notable');
		expect(iskSeverity(9_999_999_999)).toBe('notable');
		expect(iskSeverity(10_000_000_000)).toBe('severe');
	});
});

describe('utcShort', () => {
	it('reads as the EVE timer stamp, in UTC whatever the local zone', () => {
		expect(utcShort('2026-08-17T15:04:00Z')).toBe('Aug 17, 15:04');
		expect(utcShort(Date.parse('2026-01-01T00:00:00Z'))).toBe('Jan 01, 00:00');
	});

	it('crosses a month boundary by the UTC date, not the local one', () => {
		expect(utcShort('2026-02-28T23:59:00Z')).toBe('Feb 28, 23:59');
		expect(utcShort('2026-03-01T00:01:00Z')).toBe('Mar 01, 00:01');
	});
});

describe('timeAgoShort with epoch input', () => {
	it('accepts milliseconds like it accepts ISO strings', () => {
		expect(timeAgoShort(NOW.getTime() - 47 * 60_000, NOW)).toBe('47m');
		expect(timeAgoShort(NOW.getTime(), NOW)).toBe('now');
	});
});
