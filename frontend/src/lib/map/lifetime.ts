// When a wormhole is expected to die, from what the map knows about it.

import type { MapConnection } from '$lib/api/types/MapConnection';

const HOUR_MS = 3_600_000;

/** EOL means under four hours left and critical under one: a mark starts that countdown. */
const LEFT_AFTER_MARK_MS = { eol: 4 * HOUR_MS, critical: HOUR_MS } as const;

export interface LifetimeDeadline {
	/** Epoch milliseconds. */
	at: number;
	/**
	 * Before any mark the clock runs from when the hole was mapped, which is at best when it
	 * opened, so the deadline is a ceiling rather than a time.
	 */
	estimated: boolean;
}

/** When the hole should be gone, or null when nothing says: a stargate, or an unmarked hole of unknown lifetime. */
export function lifetimeDeadline(
	c: Pick<
		MapConnection,
		'kind' | 'time_status' | 'time_status_updated_at' | 'created_at' | 'lifetime_hours'
	>,
): LifetimeDeadline | null {
	if (c.kind !== 'wormhole') return null;
	if ((c.time_status === 'eol' || c.time_status === 'critical') && c.time_status_updated_at) {
		return {
			at: Date.parse(c.time_status_updated_at) + LEFT_AFTER_MARK_MS[c.time_status],
			estimated: false,
		};
	}
	if (c.lifetime_hours === null) return null;
	return { at: Date.parse(c.created_at) + c.lifetime_hours * HOUR_MS, estimated: true };
}

/** "2h 13m", "45m", and "0m" once it has run out. */
export function formatRemaining(deadlineMs: number, nowMs: number): string {
	const minutes = Math.max(0, Math.ceil((deadlineMs - nowMs) / 60_000));
	const hours = Math.floor(minutes / 60);
	const rest = minutes % 60;
	return hours === 0 ? `${rest}m` : `${hours}h ${rest}m`;
}
