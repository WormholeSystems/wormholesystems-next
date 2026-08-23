/**
 * Wrap an async fetch so only the newest call is believed: a response that arrives after
 * a later call has started is dropped instead of overwriting it, and failures are dropped
 * the same way. `cancel` drops whatever is still in flight without starting anything new.
 */
export function latest<Args extends unknown[], T>(
	load: (...args: Args) => Promise<T>,
	apply: (value: T) => void,
): { (...args: Args): void; cancel(): void } {
	let generation = 0;
	const run = (...args: Args) => {
		const request = ++generation;
		load(...args)
			.then((value) => {
				if (generation === request) apply(value);
			})
			.catch(() => {});
	};
	run.cancel = () => {
		generation++;
	};
	return run;
}
