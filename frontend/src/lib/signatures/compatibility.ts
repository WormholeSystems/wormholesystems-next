// Which of a system's signatures could be the hole you just flew through.
//
// The point is to rank, not to filter. A signature is demoted when the map says it cannot
// be the one, never hidden: signature types are typed by hand off a scanner paste, and a
// prompt that hides the row you meant to pick is worse than one that lists it last.

import type { Signature } from '$lib/api/types/Signature';
import type { SignatureTypeInfo } from '$lib/api/types/SignatureTypeInfo';

/**
 * Whether a scanned signature could be a connection at all. Only wormholes and
 * not-yet-classified signatures qualify: gas, data, relic, combat and ore are sites.
 */
export function canBeConnection(signature: Signature): boolean {
	return signature.group === 'wormhole' || signature.group === 'unknown';
}

/**
 * Whether a signature's type could lead into a system of `targetClass`.
 *
 * A type with a concrete destination class only fits that class: a hole labelled "leads
 * to nullsec" cannot have put you in a C4. An unresolved type, or one whose destination
 * is open (a bare K162), always fits.
 */
export function canLeadToClass(
	type: SignatureTypeInfo | null | undefined,
	targetClass: number | null | undefined
): boolean {
	const destination = type?.target_class;
	if (destination === null || destination === undefined) return true;
	if (targetClass === null || targetClass === undefined) return true;
	return destination === targetClass;
}

export interface SignatureGroups {
	/** Unmapped, and the type (if any) can lead to the target class. */
	likely: Signature[];
	/** Already one end of a mapped connection, so it cannot be the new hole. */
	connected: Signature[];
	/** Unmapped, but typed with a destination class that cannot match. */
	unlikely: Signature[];
}

/**
 * Sort a system's signatures into the jump prompt's three sections, in scanner-id order
 * so the list matches the order they appear in the scanner window.
 */
export function groupSignatures(
	signatures: Signature[],
	types: Map<number, SignatureTypeInfo>,
	targetClass: number | null | undefined
): SignatureGroups {
	const groups: SignatureGroups = { likely: [], connected: [], unlikely: [] };

	for (const signature of [...signatures].sort((a, b) =>
		a.signature_id.localeCompare(b.signature_id)
	)) {
		if (!canBeConnection(signature)) continue;

		if (signature.connection_id !== null) {
			groups.connected.push(signature);
		} else if (
			canLeadToClass(
				signature.signature_type_id === null
					? null
					: types.get(signature.signature_type_id),
				targetClass
			)
		) {
			groups.likely.push(signature);
		} else {
			groups.unlikely.push(signature);
		}
	}

	return groups;
}
