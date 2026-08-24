// The wall clock, ticking: "4m ago" has to stay true while the page is open. Construct
// during component init; the interval is torn down with the component.

/** A Date that refreshes every `ms`. */
export function ticking(ms: number) {
	let current = $state(new Date());
	$effect(() => {
		const timer = setInterval(() => (current = new Date()), ms);
		return () => clearInterval(timer);
	});
	return {
		get current() {
			return current;
		},
	};
}

/** The same clock as epoch milliseconds, for the second-resolution timers. */
export function tickingMs(ms = 1000) {
	let current = $state(Date.now());
	$effect(() => {
		const timer = setInterval(() => (current = Date.now()), ms);
		return () => clearInterval(timer);
	});
	return {
		get current() {
			return current;
		},
	};
}
