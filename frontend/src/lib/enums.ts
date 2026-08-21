/**
 * The value a control handed back, when it is one of the values the type allows.
 *
 * A `<select>` or a toggle group deals in `string`, while the field it feeds is a union of
 * the handful of values the server accepts. This is the one place that gap is closed, by
 * looking the value up rather than asserting it: nothing is claimed that was not found.
 */
export function oneOf<T extends string>(allowed: readonly T[], value: string): T | undefined {
	return allowed.find((option) => option === value);
}

/**
 * A table's entry for a key that may not be in it.
 *
 * Tables written as literals know exactly which keys they have, which is what makes a
 * typo a compile error. The cost is that looking one up by a value from outside — a
 * key press, a reason the server sent — no longer typechecks. This says the quiet part:
 * the answer may be missing, and the caller decides what that means.
 */
export function lookup<T>(table: Readonly<Record<string, T>>, key: string): T | undefined {
	return table[key];
}
