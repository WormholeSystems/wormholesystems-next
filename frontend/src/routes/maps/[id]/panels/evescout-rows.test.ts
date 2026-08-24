import { describe, expect, it } from 'vitest';

import type { EveScoutConnection } from '$lib/api/types/EveScoutConnection';
import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
import {
	EVESCOUT_COMPARATORS,
	bySystem,
	eveScoutTiebreak,
	latestUpdate,
	ttl,
	ttlTone,
	type EveScoutRow,
} from './evescout-rows';

const row = (system: Partial<SystemSearchResult> | undefined, jumps: number | null = null) =>
	({
		connection: {
			hub_signature: 'ABC',
			wormhole_type: null,
			remaining_hours: 5,
		} as unknown as EveScoutConnection,
		system:
			system &&
			({ wormhole_class_id: null, security: 0, name: '', ...system } as SystemSearchResult),
		route: undefined,
		jumps,
	}) as EveScoutRow;

describe('bySystem', () => {
	it('puts known space first by security descending, then wormholes by class', () => {
		const highsec = row({ security: 0.9, name: 'Jita' });
		const lowsec = row({ security: 0.3, name: 'Rancer' });
		const c2 = row({ wormhole_class_id: 2, security: -1, name: 'J1' });
		const c5 = row({ wormhole_class_id: 5, security: -1, name: 'J2' });
		expect(bySystem(highsec, lowsec)).toBeLessThan(0);
		expect(bySystem(lowsec, c2)).toBeLessThan(0);
		expect(bySystem(c2, c5)).toBeLessThan(0);
	});
});

describe('jumps ordering', () => {
	it('sorts unreachable rows last in either direction', () => {
		expect(EVESCOUT_COMPARATORS.jumps(row({}, 2), row({}, null))).toBeLessThan(0);
		expect(EVESCOUT_COMPARATORS.jumps(row({}, null), row({}, 2))).toBeGreaterThan(0);
		expect(EVESCOUT_COMPARATORS.jumps(row({}, null), row({}, null))).toBe(0);
	});
});

describe('eveScoutTiebreak', () => {
	it('falls back to class then name, so jumpless columns still read sorted', () => {
		const a = row({ security: 0.9, name: 'Amarr' });
		const b = row({ security: 0.9, name: 'Jita' });
		expect(eveScoutTiebreak(a, b)).toBeLessThan(0);
	});
});

describe('latestUpdate', () => {
	it('answers with the newest stamp, or nothing at all', () => {
		const conn = (updated_at: string | null) => ({ updated_at }) as EveScoutConnection;
		expect(latestUpdate([conn('2026-08-01T00:00:00Z'), conn('2026-08-02T00:00:00Z')])).toBe(
			'2026-08-02T00:00:00Z',
		);
		expect(latestUpdate([conn(null)])).toBeNull();
	});
});

describe('ttl', () => {
	it('reads minutes under an hour, hours above, and -- for the unknown', () => {
		expect(ttl(0.4)).toBe('24m');
		expect(ttl(0.001)).toBe('1m');
		expect(ttl(12.4)).toBe('12h');
		expect(ttl(undefined)).toBe('--');
	});

	it('turns red inside the last hour and amber inside four', () => {
		expect(ttlTone(0.5)).toContain('red');
		expect(ttlTone(3)).toContain('amber');
		expect(ttlTone(12)).toBe('text-muted-foreground');
	});
});
