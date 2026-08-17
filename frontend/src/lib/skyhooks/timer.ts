// When a skyhook's theft window is, in the terms the card talks about.
//
// One module rather than one copy per component. Legacy computes this twice, in the panel
// and in the row, with two different date parsers, and the two only agree by accident. All
// of it is pure and takes `now` as an argument, which is also what makes it testable
// without waiting two hours for a real window.

import type { Skyhook } from '$lib/api/types/Skyhook';

export type SkyhookStatus =
	/** The window has not opened yet. */
	| 'upcoming'
	/** Open, with time to spare. */
	| 'open'
	/** Open, but not for much longer. */
	| 'closing'
	/** Already over; the row should not be shown at all. */
	| 'closed';

/** How little is left before "open" becomes "hurry". */
export const CLOSING_SOON_MS = 15 * 60 * 1000;

export interface SkyhookTiming {
	status: SkyhookStatus;
	/**
	 * Milliseconds to the moment that matters: the close while it is open, the open while
	 * it is upcoming, and the (past) close once it is over.
	 */
	untilMs: number;
}

export function timing(skyhook: Skyhook, now: Date): SkyhookTiming {
	const from = new Date(skyhook.vulnerable_from).getTime();
	const until = new Date(skyhook.vulnerable_until).getTime();
	const at = now.getTime();

	if (at >= until) return { status: 'closed', untilMs: at - until };
	if (at < from) return { status: 'upcoming', untilMs: from - at };
	const left = until - at;
	return { status: left < CLOSING_SOON_MS ? 'closing' : 'open', untilMs: left };
}

/**
 * A duration as the card writes it: `<1m`, `47m`, `2h 05m`, `1d 04h`. Coarse on purpose,
 * because the number is read at a glance and a ticking second count is noise.
 */
export function formatDuration(ms: number): string {
	const minutes = Math.floor(Math.abs(ms) / 60_000);
	if (minutes < 1) return '<1m';
	if (minutes < 60) return `${minutes}m`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) {
		const rest = minutes % 60;
		return rest > 0 ? `${hours}h ${String(rest).padStart(2, '0')}m` : `${hours}h`;
	}
	const days = Math.floor(hours / 24);
	const rest = hours % 24;
	return rest > 0 ? `${days}d ${String(rest).padStart(2, '0')}h` : `${days}d`;
}

/** `12:46 – 14:46 UTC`, which is how a timer gets written down and shared. */
export function formatWindow(skyhook: Skyhook): string {
	const at = (iso: string) => {
		const d = new Date(iso);
		return `${String(d.getUTCHours()).padStart(2, '0')}:${String(d.getUTCMinutes()).padStart(2, '0')}`;
	};
	return `${at(skyhook.vulnerable_from)} – ${at(skyhook.vulnerable_until)} UTC`;
}

/** The sentence above the window in the tooltip. */
export function describe(t: SkyhookTiming): string {
	const spell = formatDuration(t.untilMs);
	if (t.status === 'closed') return `Closed ${spell} ago`;
	if (t.status === 'upcoming') return `Raidable in ${spell}`;
	return `Raidable for ${spell}`;
}

const DOT: Record<SkyhookStatus, string> = {
	upcoming: 'bg-amber-400',
	open: 'bg-emerald-400 animate-pulse',
	closing: 'bg-red-400 animate-pulse',
	closed: 'bg-muted-foreground/40'
};

const TEXT: Record<SkyhookStatus, string> = {
	upcoming: 'text-amber-400',
	open: 'text-emerald-400',
	closing: 'text-red-400 animate-pulse',
	closed: 'text-muted-foreground/60'
};

export function statusDot(status: SkyhookStatus): string {
	return DOT[status];
}

export function statusText(status: SkyhookStatus): string {
	return TEXT[status];
}
