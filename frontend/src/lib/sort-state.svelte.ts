import { readStored, sortSchema } from '$lib/storage';

/**
 * A sorted column and its direction, remembered under `key` across visits. Toggling the
 * current column flips the direction; a new column starts ascending.
 */
export function sortState<const T extends readonly string[]>(
	key: string,
	columns: T,
	initial: { column: T[number]; direction: 'asc' | 'desc' },
) {
	let current = $state(readStored(key, sortSchema(columns), initial));
	return {
		get current() {
			return current;
		},
		toggle(column: T[number]) {
			current =
				current.column === column
					? { column, direction: current.direction === 'asc' ? 'desc' : 'asc' }
					: { column, direction: 'asc' };
			localStorage.setItem(key, JSON.stringify(current));
		},
	};
}
