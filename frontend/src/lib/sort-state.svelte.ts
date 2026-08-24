import { readStored, sortSchema, writeStored } from '$lib/storage';

/**
 * A sortable list's column and direction, persisted under `key` (`null` keeps it
 * in-memory only, for lists that should forget their sort).
 */
export function sortState<const T extends readonly string[]>(
	key: string | null,
	columns: T,
	initial: { column: T[number]; direction: 'asc' | 'desc' },
) {
	let current = $state(key === null ? initial : readStored(key, sortSchema(columns), initial));
	return {
		get current() {
			return current;
		},
		toggle(column: T[number]) {
			current =
				current.column === column
					? { column, direction: current.direction === 'asc' ? 'desc' : 'asc' }
					: { column, direction: 'asc' };
			if (key !== null) writeStored(key, current);
		},
	};
}

/**
 * Rows in the sorted order a `sortState` names: the column's comparator, the direction,
 * then the undirected tiebreak so equal rows never jitter as data refreshes.
 */
export function sortedBy<Row, C extends string>(
	rows: readonly Row[],
	sort: { column: C; direction: 'asc' | 'desc' },
	comparators: Record<C, (a: Row, b: Row) => number>,
	tiebreak?: (a: Row, b: Row) => number,
): Row[] {
	const direction = sort.direction === 'asc' ? 1 : -1;
	return [...rows].sort(
		(a, b) => comparators[sort.column](a, b) * direction || (tiebreak?.(a, b) ?? 0),
	);
}
