import { describe, expect, it } from 'vitest';

import type { Signature } from '$lib/api/types/Signature';
import type { SignatureTypeInfo } from '$lib/api/types/SignatureTypeInfo';
import { matchesSignatureQuery } from './search';

const sig = (over: Partial<Signature> = {}): Signature =>
	({ signature_id: 'ABC-123', name: null, ...over }) as Signature;
const type = (over: Partial<SignatureTypeInfo> = {}): SignatureTypeInfo =>
	({ name: 'Perimeter Ambush Point', signature: 'H296', ...over }) as SignatureTypeInfo;

describe('matchesSignatureQuery', () => {
	it('matches everything on an empty or whitespace query', () => {
		expect(matchesSignatureQuery(sig(), null, '')).toBe(true);
		expect(matchesSignatureQuery(sig(), null, '   ')).toBe(true);
	});

	it('matches the scanner id, case-insensitively', () => {
		expect(matchesSignatureQuery(sig(), null, 'abc')).toBe(true);
		expect(matchesSignatureQuery(sig(), null, 'XYZ')).toBe(false);
	});

	it('matches the name when there is one', () => {
		expect(matchesSignatureQuery(sig({ name: 'Home hole' }), null, 'home')).toBe(true);
	});

	it('matches the type name and the wormhole code', () => {
		expect(matchesSignatureQuery(sig(), type(), 'ambush')).toBe(true);
		expect(matchesSignatureQuery(sig(), type(), 'h296')).toBe(true);
		expect(matchesSignatureQuery(sig(), null, 'h296')).toBe(false);
	});
});
