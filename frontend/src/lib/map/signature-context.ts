import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { Signature } from '$lib/api/types/Signature';
import type { UpdateSignature } from '$lib/api/types/UpdateSignature';

/**
 * The fields a row may change. Taken from the wire type rather than restated, so a field
 * that changes shape on the server stops compiling here; an open dictionary would have
 * accepted a misspelled key and quietly changed nothing.
 */
export type SignaturePatch = Omit<UpdateSignature, 'map_id' | 'signature_pk'>;

/**
 * What the signature row and its inputs need from the map around them: the placements and
 * edges they resolve names against, and the writes they can make.
 *
 * They used to take the whole `MapState`, which meant they could only ever be rendered on
 * a live map. Naming the surface here keeps every write in one place, lets the row be
 * tested against plain arrays, and lets the landing page render the real component from
 * static data instead of a look-alike that drifts.
 */
export interface SignatureContext {
	systems: MapSystemView[];
	connections: MapConnection[];
	sigs: Signature[];
	/** Absent in a read-only rendering; with no actions nothing can be written. */
	actions?: SignatureActions;
}

export interface SignatureActions {
	update(signaturePk: number, patch: SignaturePatch): void;
	remove(signaturePk: number): void;
	link(signaturePk: number, connectionId: number): void;
	unlink(signaturePk: number): void;
	setPreserveMass(connectionId: number, preserve: boolean): void;
}
