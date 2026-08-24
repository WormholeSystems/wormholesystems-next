import { describe, expect, it } from 'vitest';

import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { Signature } from '$lib/api/types/Signature';
import type { RouteStep } from '$lib/routing/algorithm';
import { withSignatures, wormholeSignature } from './signatures';

const system = (id: number, solarSystemId: number): MapSystemView =>
	({ kind: 'system', id, solar_system_id: solarSystemId }) as MapSystemView;
const conn = (id: number, from: number, to: number): MapConnection =>
	({ id, from_system: from, to_system: to }) as MapConnection;
const sig = (connectionId: number, solarSystemId: number, code: string): Signature =>
	({
		connection_id: connectionId,
		solar_system_id: solarSystemId,
		signature_id: code,
	}) as Signature;

const SYSTEMS = [system(1, 100), system(2, 200)];
const CONNECTIONS = [conn(10, 1, 2)];
const SIGS = [sig(10, 100, 'ABC-123'), sig(10, 200, 'XYZ-789')];

describe('wormholeSignature', () => {
	it('answers with the signature on the side being left, either direction', () => {
		expect(wormholeSignature(100, 200, SYSTEMS, CONNECTIONS, SIGS)).toBe('ABC-123');
		expect(wormholeSignature(200, 100, SYSTEMS, CONNECTIONS, SIGS)).toBe('XYZ-789');
	});

	it('is nothing without a connection, or with an unscanned departure side', () => {
		expect(wormholeSignature(100, 999, SYSTEMS, CONNECTIONS, SIGS)).toBeNull();
		expect(wormholeSignature(100, 200, SYSTEMS, CONNECTIONS, [])).toBeNull();
	});
});

describe('withSignatures', () => {
	const steps = [
		{ id: 100, via: null },
		{ id: 200, via: 'wormhole' },
		{ id: 300, via: 'gate' },
	] as unknown as RouteStep[];

	it('attaches a signature to wormhole hops only, never to the first step', () => {
		const out = withSignatures(steps, SYSTEMS, CONNECTIONS, SIGS);
		expect(out.map((s) => s.signature)).toEqual([null, 'ABC-123', null]);
	});
});
