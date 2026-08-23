// Free-text matching over a signature list, the way the jump prompt's search box filters.

import type { Signature } from '$lib/api/types/Signature';
import type { SignatureTypeInfo } from '$lib/api/types/SignatureTypeInfo';

/**
 * Whether a signature matches the query, by scanner id, name, or its type's name and
 * wormhole code. An empty query matches everything.
 */
export function matchesSignatureQuery(
	signature: Signature,
	type: SignatureTypeInfo | null,
	query: string,
): boolean {
	const q = query.trim().toLowerCase();
	if (!q) return true;
	return (
		signature.signature_id.toLowerCase().includes(q) ||
		(signature.name ?? '').toLowerCase().includes(q) ||
		(type?.name ?? '').toLowerCase().includes(q) ||
		(type?.signature ?? '').toLowerCase().includes(q)
	);
}
