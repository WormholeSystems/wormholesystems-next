// Attaching scanner signatures to route steps. Pure over the map payloads.

import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { Signature } from '$lib/api/types/Signature';
import type { RouteStep } from '$lib/routing/algorithm';
import { solarSystemId } from '$lib/map/system';

/**
 * The signature to warp to for a wormhole hop. A connection has one at each end; the one
 * that matters is on the side you are leaving, which is the one in your scanner.
 */
export function wormholeSignature(
	from: number,
	to: number,
	systems: MapSystemView[],
	connections: MapConnection[],
	sigs: Signature[],
): string | null {
	const system = new Map(systems.map((s) => [s.id, solarSystemId(s)]));
	const conn = connections.find((c) => {
		const a = system.get(c.from_system);
		const b = system.get(c.to_system);
		return (a === from && b === to) || (a === to && b === from);
	});
	if (!conn) return null;
	return (
		sigs.find((sig) => sig.connection_id === conn.id && sig.solar_system_id === from)
			?.signature_id ?? null
	);
}

/** Route steps with the signature attached to each wormhole hop. */
export function withSignatures(
	steps: RouteStep[],
	systems: MapSystemView[],
	connections: MapConnection[],
	sigs: Signature[],
): (RouteStep & { signature: string | null })[] {
	return steps.map((step, i) => ({
		...step,
		signature:
			step.via === 'wormhole' && i > 0
				? wormholeSignature(steps[i - 1].id, step.id, systems, connections, sigs)
				: null,
	}));
}
