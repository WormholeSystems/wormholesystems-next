/** `current` settles to the getter's latest value once it stops changing for `delay` ms. */
export function debounced<T>(getter: () => T, delay: number) {
	let current = $state(getter());
	$effect(() => {
		const value = getter();
		const timer = setTimeout(() => (current = value), delay);
		return () => clearTimeout(timer);
	});
	return {
		get current() {
			return current;
		},
	};
}
