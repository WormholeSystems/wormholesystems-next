import { describe, expect, it } from 'vitest';

import type { MapConnection } from '$lib/api/types/MapConnection';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { Signature } from '$lib/api/types/Signature';
import { connectionDestination } from './destination';

const sig = (connectionId: number | null): Signature =>
	({ connection_id: connectionId }) as Signature;
const conn = (id: number, from: number, to: number): MapConnection =>
	({ id, from_system: from, to_system: to }) as MapConnection;
const system = (id: number, name: string | null, alias: string | null): MapSystemView =>
	({ kind: 'system', id, name, alias }) as MapSystemView;
const ghost = (id: number, alias: string | null): MapSystemView =>
	({ kind: 'ghost', id, alias }) as MapSystemView;

describe('connectionDestination', () => {
	const systems = [system(1, 'J155207', 'home'), system(2, 'J121215', '1a'), ghost(3, '1b')];
	const connections = [conn(10, 1, 2), conn(11, 3, 1)];

	it('is nothing for an unlinked signature or a vanished connection', () => {
		expect(connectionDestination(sig(null), 1, connections, systems)).toBeNull();
		expect(connectionDestination(sig(99), 1, connections, systems)).toBeNull();
	});

	it('names the far side from either end of the connection', () => {
		expect(connectionDestination(sig(10), 1, connections, systems)).toBe('1a · J121215');
		expect(connectionDestination(sig(10), 2, connections, systems)).toBe('home · J155207');
	});

	it('falls back to the alias alone when the far side is a ghost', () => {
		expect(connectionDestination(sig(11), 1, connections, systems)).toBe('1b');
	});
});
