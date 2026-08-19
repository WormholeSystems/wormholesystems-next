// Small shared formatters for things the UI shows in more than one place.

/**
 * How long ago, coarsely: `just now`, `47m ago`, `3h ago`, `2d ago`. Takes `now` rather than
 * reading the clock, so callers that tick on a timer recompute and tests stay deterministic.
 */
export function timeAgo(iso: string, now: Date = new Date()): string {
	const seconds = Math.floor((now.getTime() - new Date(iso).getTime()) / 1000);
	if (seconds < 60) return 'just now';
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	return `${Math.floor(hours / 24)}d ago`;
}

const ISK = new Intl.NumberFormat('en-US', {
	notation: 'compact',
	compactDisplay: 'short',
	maximumFractionDigits: 1
});

/** An ISK figure as it reads on a killboard: `340M`, `1.2B`. */
export function formatIsk(value: number | null | undefined): string | null {
	if (value === null || value === undefined) return null;
	return ISK.format(value);
}

/**
 * How loudly to say a number of ISK. The thresholds are where a loss stops being routine: a
 * billion is a good ship, ten is a capital.
 */
export function iskTone(value: number | null | undefined): string {
	if (value === null || value === undefined) return 'text-muted-foreground/60';
	if (value >= 10_000_000_000) return 'font-semibold text-red-400';
	if (value >= 1_000_000_000) return 'font-semibold text-amber-400';
	return 'text-muted-foreground';
}
