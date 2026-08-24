// One write path for a connection's degradable properties, wherever the edit comes from
// (the context menu, the popover, or a signature row). Each property keeps its own action
// copy, so the toasts still say what actually changed.

import { api } from '$lib/api/client';
import type { SetConnectionStatus } from '$lib/api/types/SetConnectionStatus';
import type { MapAction } from '$lib/map/actions';

interface ConnectionHost {
	mapId: number;
	run(action: MapAction, promise: Promise<unknown>, detail?: string): void;
}

export type ConnectionPatch = Omit<SetConnectionStatus, 'map_id' | 'connection_id'>;

const ACTION_FOR: Record<string, MapAction> = {
	kind: 'setConnectionType',
	mass_status: 'setConnectionMass',
	time_status: 'setConnectionLifetime',
	size: 'setConnectionSize',
	preserve_mass: 'setPreserveMass',
};

export function patchConnection(
	map: ConnectionHost,
	connectionId: number,
	patch: ConnectionPatch,
): void {
	const named = Object.keys(patch).find((k) => k in ACTION_FOR);
	map.run(
		ACTION_FOR[named ?? 'kind'],
		api.setConnectionStatus({ map_id: map.mapId, connection_id: connectionId, ...patch }),
	);
}
