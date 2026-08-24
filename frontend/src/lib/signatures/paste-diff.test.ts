import { describe, expect, it } from 'vitest';

import type { Signature } from '$lib/api/types/Signature';
import { deletedByPaste, pasteStatus } from './paste-diff';

const sig = (id: string): Signature => ({ signature_id: id }) as Signature;

describe('pasteStatus', () => {
	const preIds = new Set(['ABC-123']);
	const pastedIds = new Set(['ABC-123', 'DEF-456']);

	it('is quiet while no paste is being reviewed', () => {
		expect(pasteStatus(sig('ABC-123'), preIds, null)).toBeNull();
	});

	it('classifies updated, new, and deleted rows', () => {
		expect(pasteStatus(sig('ABC-123'), preIds, pastedIds)).toBe('updated');
		expect(pasteStatus(sig('DEF-456'), preIds, pastedIds)).toBe('new');
		expect(pasteStatus(sig('GHI-789'), preIds, pastedIds)).toBe('deleted');
	});

	it('stays stable after the round-trip creates the pasted rows', () => {
		// The snapshot was taken before the paste; the fresh row keeps reading as new even
		// once the refetch has made it exist server-side.
		expect(pasteStatus(sig('DEF-456'), preIds, pastedIds)).toBe('new');
	});
});

describe('deletedByPaste', () => {
	it('lists what the scan no longer sees, and nothing outside a review', () => {
		const sigs = [sig('ABC-123'), sig('GHI-789')];
		expect(deletedByPaste(sigs, new Set(['ABC-123']))).toEqual([sig('GHI-789')]);
		expect(deletedByPaste(sigs, null)).toEqual([]);
	});
});
