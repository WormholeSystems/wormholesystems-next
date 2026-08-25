import { describe, expect, it } from 'vitest';

import { classMeta, destClassMeta, effectTextColor, isWormholeClass } from './classes';

describe('classMeta', () => {
	it('answers by class id when one is known', () => {
		expect(classMeta(5, null).short).toBe('C5');
		expect(classMeta(25, 0.8).short).toBe('P');
	});

	it('falls back to the CCP-rounded security band when there is no class id', () => {
		expect(classMeta(null, 0.9).short).toBe('H');
		expect(classMeta(null, 0.45).short).toBe('H');
		expect(classMeta(null, 0.3).short).toBe('L');
		// Syndicate-style NPC nullsec sits just below zero.
		expect(classMeta(null, -0.014694).short).toBe('N');
		expect(classMeta(null, 0.02).short).toBe('L');
		expect(classMeta(null, 0).short).toBe('N');
		expect(classMeta(null, -0.4).short).toBe('N');
	});

	it('reads as unknown with neither, like a ghost', () => {
		expect(classMeta(null, null).short).toBe('?');
		expect(classMeta(null, null).sortWeight).toBe(99);
	});

	it('sorts known space before wormhole space', () => {
		expect(classMeta(7, null).sortWeight).toBeLessThan(classMeta(1, null).sortWeight);
	});
});

describe('isWormholeClass', () => {
	it('is true for wormhole classes, drifter holes included', () => {
		expect(isWormholeClass(1)).toBe(true);
		expect(isWormholeClass(13)).toBe(true);
		expect(isWormholeClass(14)).toBe(true);
	});

	it('is false for known space, Pochven and null', () => {
		expect(isWormholeClass(7)).toBe(false);
		expect(isWormholeClass(25)).toBe(false);
		expect(isWormholeClass(null)).toBe(false);
	});
});

describe('destClassMeta', () => {
	it('reads a static destination, or unknown when there is none', () => {
		expect(destClassMeta(6).short).toBe('C6');
		expect(destClassMeta(null).short).toBe('?');
	});
});

describe('effectTextColor', () => {
	it('matches on keyword, however the effect is spelled', () => {
		expect(effectTextColor('Pulsar')).toBe('text-blue-400');
		expect(effectTextColor('Wolf-Rayet Star')).toBe('text-amber-600');
		expect(effectTextColor('Cataclysmic Variable')).toBe('text-yellow-400');
	});

	it('mutes anything it does not know, null included', () => {
		expect(effectTextColor('Quasar')).toBe('text-muted-foreground');
		expect(effectTextColor(null)).toBe('text-muted-foreground');
	});
});
