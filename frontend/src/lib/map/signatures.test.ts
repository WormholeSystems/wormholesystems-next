import { describe, expect, it } from 'vitest';

import { parseScan } from './signatures';
import type { SignatureCatalog } from '$lib/api/types/SignatureCatalog';
import type { SignatureTypeInfo } from '$lib/api/types/SignatureTypeInfo';

function type(
	partial: Partial<SignatureTypeInfo> &
		Pick<SignatureTypeInfo, 'id' | 'name' | 'signature_category_id'>,
): SignatureTypeInfo {
	return {
		signature: null,
		target_class: null,
		extra: null,
		spawn_areas: [],
		total_mass: null,
		max_jump_mass: null,
		lifetime_hours: null,
		signature_strength: null,
		...partial,
	};
}

// Category ids follow the seeded catalog: 1 wormhole, 2 data, 3 relic, 5 gas.
const catalog: SignatureCatalog = {
	categories: [
		{ id: 1, name: 'Wormhole', code: 'WH' },
		{ id: 2, name: 'Data Site', code: 'DATA' },
		{ id: 3, name: 'Relic Site', code: 'RELIC' },
		{ id: 5, name: 'Gas Site', code: 'GAS' },
	],
	types: [
		type({ id: 10, name: 'Unstable Wormhole', signature_category_id: 1, signature: 'K162' }),
		type({ id: 20, name: 'Unsecured Frontier Receiver', signature_category_id: 2 }),
		type({ id: 30, name: 'Forgotten Perimeter Coronation Site', signature_category_id: 3 }),
	],
};

describe('parseScan', () => {
	it('parses a multi-line scanner paste', () => {
		const paste = [
			'ABC-123\tCosmic Signature\tWormhole\tUnstable Wormhole\t100.0%\t2.5 AU',
			'DEF-456\tCosmic Signature\tData Site\tUnsecured Frontier Receiver\t97.2%\t11.1 AU',
			'GHI-789\tCosmic Signature\tRelic Site\tForgotten Perimeter Coronation Site\t54.8%\t3.2 AU',
		].join('\n');

		expect(parseScan(paste, catalog)).toEqual([
			// A wormhole's type is never taken from the paste: the scanner only ever says
			// "Unstable Wormhole", so the raw name is kept and the user picks the type.
			{
				signature_id: 'ABC-123',
				group: 'wormhole',
				signature_type_id: undefined,
				name: 'Unstable Wormhole',
			},
			{ signature_id: 'DEF-456', group: 'data', signature_type_id: 20, name: undefined },
			{ signature_id: 'GHI-789', group: 'relic', signature_type_id: 30, name: undefined },
		]);
	});

	it('keeps partially scanned rows: no category, or a category with no type yet', () => {
		const paste = [
			'JKL-012\tCosmic Signature\t\t\t2.9%\t8.7 AU',
			'MNO-345\tCosmic Signature\tGas Site\t\t12.0%\t1.4 AU',
		].join('\n');

		expect(parseScan(paste, catalog)).toEqual([
			{ signature_id: 'JKL-012', group: undefined, signature_type_id: undefined, name: undefined },
			{ signature_id: 'MNO-345', group: 'gas', signature_type_id: undefined, name: undefined },
		]);
	});

	it('keeps an unmatched type name as the raw name', () => {
		const rows = parseScan(
			'PQR-678\tCosmic Signature\tData Site\tSome New Site\t80%\t1 AU',
			catalog,
		);
		expect(rows).toEqual([
			{
				signature_id: 'PQR-678',
				group: 'data',
				signature_type_id: undefined,
				name: 'Some New Site',
			},
		]);
	});

	it('matches a category by its " - " segments when the full cell is not a category name', () => {
		const rows = parseScan('STU-901\tCosmic Signature\tGuristas - Data Site\t\t9%\t3 AU', catalog);
		expect(rows).toEqual([
			{ signature_id: 'STU-901', group: 'data', signature_type_id: undefined, name: undefined },
		]);
	});

	it('skips malformed lines and keeps the rest', () => {
		const paste = [
			'',
			'not a scanner line at all',
			'AB-12\tCosmic Signature\tWormhole\tUnstable Wormhole\t100%\t1 AU',
			'ABC-123\tCosmic Signature',
			'VWX-234\tCosmic Signature\tRelic Site\tForgotten Perimeter Coronation Site\t54.8%\t3.2 AU',
		].join('\n');

		expect(parseScan(paste, catalog)).toEqual([
			{ signature_id: 'VWX-234', group: 'relic', signature_type_id: 30, name: undefined },
		]);
	});

	it('parses nothing from empty input', () => {
		expect(parseScan('', catalog)).toEqual([]);
	});
});
