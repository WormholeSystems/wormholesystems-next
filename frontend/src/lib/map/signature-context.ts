import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { Signature } from '$lib/api/types/Signature';
import type { UpdateSignature } from '$lib/api/types/UpdateSignature';

import { api } from '$lib/api/client';
import type { MapAction } from '$lib/map/actions';
import { patchConnection } from '$lib/map/connection-actions';

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

interface SignatureHost {
	mapId: number;
	readonly systems: MapSystemView[];
	readonly connections: MapConnection[];
	readonly sigs: Signature[];
	run(action: MapAction, promise: Promise<unknown>, detail?: string): void;
}

/**
 * The live context over a map. The rows and their inputs take this rather than the whole
 * map, so the same components render from static data elsewhere; every write they can
 * make is named here.
 */
export function makeSignatureContext(map: SignatureHost): SignatureContext {
	return {
		get systems() {
			return map.systems;
		},
		get connections() {
			return map.connections;
		},
		get sigs() {
			return map.sigs;
		},
		actions: {
			update: (signature_pk, patch) =>
				map.run(
					'updateSignature',
					api.updateSignature({ map_id: map.mapId, signature_pk, ...patch }),
				),
			remove: (signature_pk) =>
				map.run('removeSignature', api.removeSignature({ map_id: map.mapId, signature_pk })),
			link: (signature_pk, connection_id) =>
				map.run(
					'linkSignature',
					api.linkSignature({ map_id: map.mapId, signature_pk, connection_id }),
				),
			unlink: (signature_pk) =>
				map.run('unlinkSignature', api.unlinkSignature({ map_id: map.mapId, signature_pk })),
			setPreserveMass: (connection_id, preserve_mass) =>
				patchConnection(map, connection_id, { preserve_mass }),
		},
	};
}
