import { describe, expect, it } from 'vitest';

import { aliasTargetKind, guessNextAlias, isIgnoredAlias, suggestAlias } from './alias';

describe('guessNextAlias, numeric', () => {
	it('numbers the first systems from the root', () => {
		expect(guessNextAlias(null, [])).toBe('1');
		expect(guessNextAlias(null, ['1', '2'])).toBe('3');
	});

	it('extends the parent, so the alias carries the path', () => {
		expect(guessNextAlias('1', ['1'])).toBe('11');
		expect(guessNextAlias('12', ['1', '12'])).toBe('121');
	});

	it('fills a gap left by a deleted system before growing', () => {
		expect(guessNextAlias(null, ['1', '3', '4'])).toBe('2');
	});

	it('does not mistake a grandchild for a direct child', () => {
		// `11` and `111` both extend `1`, but only `11` is a child of it.
		expect(guessNextAlias('1', ['1', '11', '111'])).toBe('12');
	});

	it('matches case-insensitively and suggests upper case', () => {
		expect(guessNextAlias('a', ['a', 'a1'])).toBe('A2');
	});
});

describe('guessNextAlias, alphabetical', () => {
	const alpha = { scheme: 'alphabetical' as const };

	it('walks the letters', () => {
		expect(guessNextAlias('A', ['A'], alpha)).toBe('AA');
		expect(guessNextAlias('A', ['A', 'AA', 'AB'], alpha)).toBe('AC');
	});

	it('skips the letters reserved for k-space exits', () => {
		// G is followed by I: H, L, N and P are reserved.
		expect(guessNextAlias('', ['A', 'B', 'C', 'D', 'E', 'F', 'G'], alpha)).toBe('I');
	});

	it('numbers k-space exits per reserved letter', () => {
		expect(guessNextAlias('A', ['A'], { ...alpha, targetKind: 'h' })).toBe('AH1');
		expect(guessNextAlias('A', ['A', 'AH1'], { ...alpha, targetKind: 'h' })).toBe('AH2');
		// Each letter counts separately, so a highsec exit does not push the lowsec one along.
		expect(guessNextAlias('A', ['A', 'AH1'], { ...alpha, targetKind: 'l' })).toBe('AL1');
	});

	it('does not count a branch off a k-space exit as another exit', () => {
		expect(guessNextAlias('A', ['A', 'AH1', 'AH1A'], { ...alpha, targetKind: 'h' })).toBe('AH2');
	});

	it('only counts direct children, not deeper ones', () => {
		expect(guessNextAlias('A', ['A', 'AA', 'AAB'], alpha)).toBe('AB');
	});
});

describe('the ignored alias', () => {
	it('matches trimmed and case-insensitively, and is off when unset', () => {
		expect(isIgnoredAlias(' home ', 'HOME')).toBe(true);
		expect(isIgnoredAlias('HOME', '')).toBe(false);
		expect(isIgnoredAlias('A', 'HOME')).toBe(false);
	});

	it('starts a fresh sequence, since home is not a chain node', () => {
		expect(guessNextAlias('HOME', ['HOME', '1'], { ignoredAlias: 'HOME' })).toBe('2');
	});
});

describe('suggestAlias', () => {
	const base = { aliases: ['1'], parentAlias: '1' };

	it('names a wormhole reached from anywhere', () => {
		expect(
			suggestAlias({ ...base, targetIsWormhole: true, originIsWormhole: false })
		).toBe('11');
	});

	it('continues the chain into k-space from an aliased hole', () => {
		expect(
			suggestAlias({ ...base, targetIsWormhole: false, originIsWormhole: true })
		).toBe('11');
	});

	it('leaves plain travel unnamed', () => {
		// K-space to k-space with nothing aliased is not part of any chain.
		expect(
			suggestAlias({
				parentAlias: null,
				aliases: [],
				targetIsWormhole: false,
				originIsWormhole: false
			})
		).toBeNull();
	});
});

describe('aliasTargetKind', () => {
	it('reserves a letter for k-space and leaves wormholes plain', () => {
		expect(aliasTargetKind(true, 'C5')).toBe('wormhole');
		expect(aliasTargetKind(false, 'H')).toBe('h');
		expect(aliasTargetKind(false, 'C5')).toBeUndefined();
	});
});
