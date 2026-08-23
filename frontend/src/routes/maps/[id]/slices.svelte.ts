// Refetching in slices: the map screen is several independently-refetchable halves, and a
// socket frame names which of them it invalidated.

import type { MapEvent } from '$lib/api/types/MapEvent';

/** The independently-refetchable halves of what a map screen shows. */
export const SLICES = ['graph', 'signatures', 'watchlist', 'history', 'stale'] as const;
export type Slice = (typeof SLICES)[number] | 'characters';

/**
 * How long to wait before acting on a frame. Long enough to swallow the burst one write
 * produces, short enough that nobody watching the map notices the delay.
 */
const BURST_MS = 60;

/**
 * What one frame off the map socket actually invalidates.
 *
 * The event says what changed and this believes it, rather than reloading the map five
 * ways over. Two things ride along wider than they look: the graph follows a signature
 * change because `ghost::reconcile` can raise or drop a node as a side effect of one,
 * and the history follows any command because the journal grows without announcing it.
 */
export function slicesFor(event: MapEvent): Slice[] {
	switch (event.type) {
		case 'characters_changed':
			return ['characters'];
		// A kill changes nothing about the graph; only the killmail card reacts, off the
		// tick the caller keeps.
		case 'killmail_received':
			return [];
		case 'watchlist_changed':
			return ['watchlist'];
		// Undo, redo and jumping to a step all publish this, and moving the cursor can
		// touch anything the steps it crosses did — the server says so where it
		// publishes it. So this one really does mean everything.
		case 'history_changed':
			return [...SLICES];
		case 'signature_changed':
			return ['signatures', 'graph', 'history'];
		case 'connection_changed':
			return ['graph', 'stale', 'history'];
		case 'map_updated':
		case 'access_changed':
			return ['graph'];
		default:
			return ['graph', 'history'];
	}
}

/**
 * Burst coalescing around a fetch-per-slice. The server is chatty in bursts: pasting a
 * scan publishes a frame per system, per connection and per placement it touched, all
 * within a few milliseconds. The timer collapses the burst into one request, and the
 * in-flight set stops a second one overlapping the first — with a single re-run queued
 * behind it, so a change that landed mid-request is not missed.
 */
export class SliceFetcher {
	private fetch: (slice: Slice) => Promise<void>;
	private timers = new Map<Slice, ReturnType<typeof setTimeout>>();
	private fetching = new Set<Slice>();
	private refetchAfter = new Set<Slice>();

	constructor(fetch: (slice: Slice) => Promise<void>) {
		this.fetch = fetch;
	}

	/** Ask for a slice, soon. */
	schedule(slice: Slice) {
		clearTimeout(this.timers.get(slice));
		this.timers.set(
			slice,
			setTimeout(() => {
				this.timers.delete(slice);
				void this.run(slice);
			}, BURST_MS),
		);
	}

	private async run(slice: Slice) {
		if (this.fetching.has(slice)) {
			this.refetchAfter.add(slice);
			return;
		}
		this.fetching.add(slice);
		try {
			await this.fetch(slice);
		} finally {
			this.fetching.delete(slice);
			if (this.refetchAfter.delete(slice)) this.schedule(slice);
		}
	}
}
