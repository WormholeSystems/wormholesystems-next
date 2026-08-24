<script lang="ts">
	// An inline ship-type search (Command-driven: arrow keys + Enter to pick), with the
	// ship's icon and group. Results come from the local SDE search endpoint.
	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';

	import { q } from '$lib/api/queries';
	import type { ShipSearchResult } from '$lib/api/types/ShipSearchResult';
	import * as Command from '$lib/components/ui/command';
	import EveImage from '$lib/components/EveImage.svelte';
	import { debounced } from '$lib/debounced.svelte';

	let {
		onpick,
		testid = 'ship-search',
	}: {
		onpick: (ship: ShipSearchResult) => void;
		testid?: string;
	} = $props();

	let term = $state('');
	const settled = debounced(() => term.trim(), 200);
	// Keyed by term, so a slow reply can never land on a newer search; the previous list
	// stays painted while the next one fetches.
	const search = createQuery(() => ({
		...q.searchShips(settled.current),
		enabled: settled.current.length > 0,
		placeholderData: keepPreviousData,
	}));
	const results = $derived(settled.current.length > 0 ? (search.data ?? []) : []);
</script>

<Command.Root shouldFilter={false} class="rounded-md border bg-transparent">
	<Command.Input
		placeholder="Search ship type…"
		bind:value={term}
		data-testid={testid}
		class="h-7 text-xs"
	/>
	{#if term.trim().length > 0}
		<Command.List class="max-h-40">
			<Command.Empty>No matches</Command.Empty>
			<Command.Group>
				{#each results as ship (ship.id)}
					<Command.Item value={String(ship.id)} onSelect={() => onpick(ship)} class="text-xs">
						<EveImage kind="type" id={ship.id} class="size-4 rounded" />
						<span class="truncate">{ship.name}</span>
						<span class="ml-auto shrink-0 text-xs text-muted-foreground">{ship.group_name}</span>
					</Command.Item>
				{/each}
			</Command.Group>
		</Command.List>
	{/if}
</Command.Root>
