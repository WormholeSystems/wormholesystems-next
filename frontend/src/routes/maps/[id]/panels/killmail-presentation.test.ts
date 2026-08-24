import { describe, expect, it } from 'vitest';

import type { MapKillmail } from '$lib/api/types/MapKillmail';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import { crowdLabel, crowdTone, partyName, partyOrg, systemKey } from './killmail-presentation';

const kill = (over: Partial<MapKillmail>): MapKillmail =>
	({ is_npc: false, is_solo: false, attacker_count: 4, ...over }) as MapKillmail;
const victim = (over: Partial<MapKillmail['victim']>): MapKillmail['victim'] =>
	({
		character_name: null,
		alliance_ticker: null,
		corporation_ticker: null,
		alliance_name: null,
		corporation_name: null,
		...over,
	}) as MapKillmail['victim'];

describe('crowd tone and label', () => {
	it('mutes NPC kills, highlights solo hunters, counts the rest', () => {
		expect(crowdTone(kill({ is_npc: true }))).toContain('/50');
		expect(crowdLabel(kill({ is_npc: true }))).toBe('Killed by NPCs');
		expect(crowdTone(kill({ is_solo: true }))).toContain('amber');
		expect(crowdLabel(kill({ is_solo: true }))).toBe('Solo kill');
		expect(crowdLabel(kill({}))).toBe('4 attackers');
	});
});

describe('partyName and partyOrg', () => {
	it('names the pilot with the loudest ticker it has', () => {
		expect(partyName(victim({ character_name: 'Pilot', alliance_ticker: 'HK' }))).toBe(
			'Pilot [HK]',
		);
		expect(partyName(victim({ character_name: 'Pilot', corporation_ticker: 'CORP' }))).toBe(
			'Pilot [CORP]',
		);
		expect(partyName(victim({}))).toBe('Unknown pilot');
	});

	it('spells out the organisation, whichever halves exist', () => {
		expect(partyOrg(victim({ corporation_name: 'C', alliance_name: 'A' }))).toBe('C · A');
		expect(partyOrg(victim({ alliance_name: 'A' }))).toBe('A');
		expect(partyOrg(victim({}))).toBeNull();
	});
});

describe('systemKey', () => {
	const system = (id: number, solar: number) =>
		({ kind: 'system', id, solar_system_id: solar }) as MapSystemView;
	const ghost = (id: number) => ({ kind: 'ghost', id }) as MapSystemView;

	it('is stable across ordering and ignores ghosts', () => {
		expect(systemKey([system(2, 200), ghost(3), system(1, 100)])).toBe('100,200');
		expect(systemKey([system(1, 100), system(2, 200)])).toBe('100,200');
		expect(systemKey([])).toBe('');
	});
});
