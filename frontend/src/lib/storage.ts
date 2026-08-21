import * as v from 'valibot';
import { browser } from '$app/environment';

/**
 * Read a value the browser is holding for us, and only believe it if it still looks right.
 *
 * What is in `localStorage` is not ours: a user can edit it, and a value written by an
 * older version of the app can outlive the shape it was written for. Neither should reach
 * the rest of the app, and neither is worth an error — the fallback is what a first visit
 * would have got.
 */
export function readStored<T>(
	key: string,
	schema: v.GenericSchema<T>,
	// `NoInfer`, so the shape comes from the schema alone: a fallback written inline would
	// otherwise widen it to whatever its literals happen to be.
	fallback: NoInfer<T>,
): T {
	if (!browser) return fallback;
	const raw = localStorage.getItem(key);
	if (raw === null) return fallback;
	try {
		const parsed = v.safeParse(schema, JSON.parse(raw));
		return parsed.success ? parsed.output : fallback;
	} catch {
		// Not JSON at all.
		return fallback;
	}
}

/** A sorted column and its direction, which is what two of the panels remember. */
export function sortSchema<const T extends readonly string[]>(columns: T) {
	return v.object({
		column: v.picklist(columns),
		direction: v.picklist(['asc', 'desc'] as const),
	});
}
