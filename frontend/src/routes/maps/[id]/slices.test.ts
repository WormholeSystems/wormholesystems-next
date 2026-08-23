import { describe, expect, it } from 'vitest';

import type { MapEvent } from '$lib/api/types/MapEvent';
import { SLICES, slicesFor } from './slices.svelte';

const event = (type: MapEvent['type']): MapEvent => ({ type, map_id: 1 }) as MapEvent;

describe('slicesFor', () => {
	it('routes presence to the characters slice alone', () => {
		expect(slicesFor(event('characters_changed'))).toEqual(['characters']);
	});

	it('routes a kill to nothing; the killmail tick is not a slice', () => {
		expect(slicesFor(event('killmail_received'))).toEqual([]);
	});

	it('routes a watchlist change to the watchlist alone', () => {
		expect(slicesFor(event('watchlist_changed'))).toEqual(['watchlist']);
	});

	it('routes a history change to every slice', () => {
		expect(slicesFor(event('history_changed'))).toEqual([...SLICES]);
	});

	it('drags the graph along with a signature change, for ghost reconciliation', () => {
		expect(slicesFor(event('signature_changed'))).toEqual(['signatures', 'graph', 'history']);
	});

	it('drags the stale list along with a connection change', () => {
		expect(slicesFor(event('connection_changed'))).toEqual(['graph', 'stale', 'history']);
	});

	it('routes map and access changes to the graph alone', () => {
		expect(slicesFor(event('map_updated'))).toEqual(['graph']);
		expect(slicesFor(event('access_changed'))).toEqual(['graph']);
	});

	it('routes every structural change to graph and history', () => {
		for (const type of [
			'system_added',
			'system_moved',
			'system_removed',
			'system_details_changed',
		] as const) {
			expect(slicesFor(event(type))).toEqual(['graph', 'history']);
		}
	});
});
