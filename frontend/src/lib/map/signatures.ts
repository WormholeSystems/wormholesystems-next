// Signature catalog access, category metadata, and the scanner-paste parser
// (legacy ports). The catalog comes from `/api/signature-types` and is cached for the
// session; category metadata mirrors the legacy icon/color vocabulary.

import CircleHelpIcon from '@lucide/svelte/icons/circle-help';
import CloudIcon from '@lucide/svelte/icons/cloud';
import DatabaseIcon from '@lucide/svelte/icons/database';
import FanIcon from '@lucide/svelte/icons/fan';
import GemIcon from '@lucide/svelte/icons/gem';
import LandmarkIcon from '@lucide/svelte/icons/landmark';
import ShieldIcon from '@lucide/svelte/icons/shield';
import SwordsIcon from '@lucide/svelte/icons/swords';

import { api } from '$lib/api/client';
import type { PastedSignature } from '$lib/api/types/PastedSignature';
import type { SignatureCatalog } from '$lib/api/types/SignatureCatalog';
import type { SignatureGroup } from '$lib/api/types/SignatureGroup';
import type { SignatureTypeInfo } from '$lib/api/types/SignatureTypeInfo';
import { destClassMeta } from './classes';

export interface CategoryMeta {
	group: SignatureGroup;
	categoryId: number | null;
	label: string;
	abbrev: string;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	icon: any;
	color: string;
}

/** Legacy category vocabulary, in the legacy filter order. `unknown` = uncategorized. */
export const CATEGORIES: CategoryMeta[] = [
	{ group: 'wormhole', categoryId: 1, label: 'Wormhole', abbrev: 'WH', icon: FanIcon, color: 'text-sky-400' },
	{ group: 'data', categoryId: 2, label: 'Data Site', abbrev: 'Data', icon: DatabaseIcon, color: 'text-cyan-400' },
	{ group: 'relic', categoryId: 3, label: 'Relic Site', abbrev: 'Relic', icon: LandmarkIcon, color: 'text-amber-400' },
	{ group: 'ore', categoryId: 6, label: 'Ore Site', abbrev: 'Ore', icon: GemIcon, color: 'text-yellow-400' },
	{ group: 'gas', categoryId: 5, label: 'Gas Site', abbrev: 'Gas', icon: CloudIcon, color: 'text-orange-400' },
	{ group: 'combat', categoryId: 4, label: 'Combat Site', abbrev: 'Combat', icon: SwordsIcon, color: 'text-green-400' },
	{ group: 'homefront', categoryId: 7, label: 'Homefront Operations', abbrev: 'HF', icon: ShieldIcon, color: 'text-rose-400' },
	{
		group: 'unknown',
		categoryId: null,
		label: 'Uncategorized',
		abbrev: '—',
		icon: CircleHelpIcon,
		color: 'text-muted-foreground'
	}
];

export function categoryMeta(group: SignatureGroup): CategoryMeta {
	return CATEGORIES.find((c) => c.group === group) ?? CATEGORIES[CATEGORIES.length - 1];
}

export function groupForCategoryId(categoryId: number): SignatureGroup {
	return CATEGORIES.find((c) => c.categoryId === categoryId)?.group ?? 'unknown';
}

let catalogPromise: Promise<SignatureCatalog> | null = null;

/** The signature type catalog, fetched once per session. */
export function loadCatalog(): Promise<SignatureCatalog> {
	catalogPromise ??= api.signatureCatalog();
	return catalogPromise;
}

/**
 * Types of one category that can spawn in a system of the given class, sorted by their
 * destination class (wormholes) with sites in catalog order.
 */
export function typesForCategory(
	catalog: SignatureCatalog,
	categoryId: number,
	systemClassId: number | null
): SignatureTypeInfo[] {
	return catalog.types
		.filter(
			(t) =>
				t.signature_category_id === categoryId &&
				(systemClassId === null || t.spawn_areas.includes(systemClassId))
		)
		.toSorted(
			(a, b) => destClassMeta(a.target_class).sortWeight - destClassMeta(b.target_class).sortWeight
		);
}

export function typeById(catalog: SignatureCatalog, id: number | null): SignatureTypeInfo | null {
	return id === null ? null : (catalog.types.find((t) => t.id === id) ?? null);
}

/**
 * Parse a pasted in-game probe scan (legacy SignatureParser port). Tab-separated rows:
 * `id, scan group (ignored), category, type name, signal %, distance`. Rows without at
 * least 4 columns or a 7-char id are skipped. The category matches by exact name, then
 * by ` - ` segments (first known segment wins). The type matches by exact name within
 * the category — never for wormholes (a hole's type is chosen by the user, not the
 * scanner). An unmatched type name is kept as the raw `name`.
 */
export function parseScan(text: string, catalog: SignatureCatalog): PastedSignature[] {
	const out: PastedSignature[] = [];
	for (const line of text.split('\n')) {
		const cols = line.split('\t');
		if (cols.length < 4) continue;
		const sid = (cols[0] ?? '').trim();
		if (sid.length !== 7) continue;

		const category = matchCategory(catalog, (cols[2] ?? '').trim());
		const typeName = (cols[3] ?? '').trim();
		let typeId: number | null = null;
		if (category !== null && groupForCategoryId(category) !== 'wormhole' && typeName) {
			typeId =
				catalog.types.find((t) => t.signature_category_id === category && t.name === typeName)
					?.id ?? null;
		}
		out.push({
			signature_id: sid,
			group: category === null ? undefined : groupForCategoryId(category),
			signature_type_id: typeId ?? undefined,
			name: category !== null && typeId === null && typeName ? typeName : undefined
		});
	}
	return out;
}

function matchCategory(catalog: SignatureCatalog, name: string): number | null {
	if (!name) return null;
	const exact = catalog.categories.find((c) => c.name === name);
	if (exact) return exact.id;
	for (const segment of name.split(' - ')) {
		const hit = catalog.categories.find((c) => c.name === segment.trim());
		if (hit) return hit.id;
	}
	return null;
}
