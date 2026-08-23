// Where an already-connected signature's hole leads, so a stale one is recognisable.

import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { Signature } from '$lib/api/types/Signature';
import { systemName } from '$lib/map/system';

/**
 * The label for the far side of the signature's connection, seen from the placement
 * `originId`: `alias · name`, whichever half is known, or null when nothing is.
 */
export function connectionDestination(
	signature: Signature,
	originId: number,
	connections: MapConnection[],
	systems: MapSystemView[],
): string | null {
	if (signature.connection_id === null) return null;
	const connection = connections.find((c) => c.id === signature.connection_id);
	if (!connection) return null;
	const otherId =
		connection.from_system === originId ? connection.to_system : connection.from_system;
	const other = systems.find((s) => s.id === otherId);
	if (!other) return null;
	const name = systemName(other);
	if (!name) return other.alias;
	return other.alias ? `${other.alias} · ${name}` : name;
}
