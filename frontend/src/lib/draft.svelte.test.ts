import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';

import { draft } from './draft.svelte';

function withRoot(run: () => void) {
	const cleanup = $effect.root(run);
	cleanup();
}

describe('draft', () => {
	it('starts as a copy of the saved value and tracks edits as dirty', () => {
		withRoot(() => {
			const buffer = draft(() => ({ name: 'Home' }));
			expect(buffer.value).toEqual({ name: 'Home' });
			expect(buffer.dirty).toBe(false);
			buffer.value.name = 'Staging';
			expect(buffer.dirty).toBe(true);
		});
	});

	it('reseeds when the saved value changes underneath', () => {
		withRoot(() => {
			let saved = $state({ name: 'Home' });
			const buffer = draft(() => saved);
			flushSync();
			saved = { name: 'Renamed elsewhere' };
			flushSync();
			expect(buffer.value).toEqual({ name: 'Renamed elsewhere' });
		});
	});

	it('does not clobber typing when an identical refetch lands', () => {
		withRoot(() => {
			let saved = $state({ name: 'Home' });
			const buffer = draft(() => saved);
			flushSync();
			buffer.value.name = 'Half-typ';
			// A refetch returns a new object with the same content.
			saved = { name: 'Home' };
			flushSync();
			expect(buffer.value.name).toBe('Half-typ');
		});
	});

	it('reset returns to the saved value and clears dirty', () => {
		withRoot(() => {
			const buffer = draft(() => ({ name: 'Home' }));
			buffer.value.name = 'Oops';
			buffer.reset();
			expect(buffer.value).toEqual({ name: 'Home' });
			expect(buffer.dirty).toBe(false);
		});
	});
});
