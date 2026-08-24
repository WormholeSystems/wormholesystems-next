/**
 * An editing buffer seeded from server state. Reseeds only when the saved value changes
 * underneath (a reload, or someone else's edit), so an identical refetch never clobbers
 * what is being typed. Component init only.
 */
export function draft<T>(
	saved: () => T,
	opts: { equals?: (a: T, b: T) => boolean; clone?: (v: T) => T } = {},
) {
	const equals = opts.equals ?? ((a: T, b: T) => JSON.stringify(a) === JSON.stringify(b));
	const clone = opts.clone ?? ((v: T) => structuredClone(v));

	let value = $state(clone(saved()));
	let seeded = saved();

	$effect(() => {
		const next = saved();
		if (equals(next, seeded)) return;
		seeded = next;
		value = clone(next);
	});

	return {
		get value() {
			return value;
		},
		set value(v: T) {
			value = v;
		},
		get dirty() {
			return !equals(value, saved());
		},
		reset() {
			seeded = saved();
			value = clone(seeded);
		},
	};
}
