<script lang="ts">
	// An inline ship-type search (Command-driven: arrow keys + Enter to pick), with the
	// ship's icon and group. Results come from the local SDE search endpoint.
	import { api } from '$lib/api/client';
	import { latest } from '$lib/latest';
	import type { ShipSearchResult } from '$lib/api/types/ShipSearchResult';
	import * as Command from '$lib/components/ui/command';
	import EveImage from '$lib/components/EveImage.svelte';

	let {
		onpick,
		testid = 'ship-search',
	}: {
		onpick: (ship: ShipSearchResult) => void;
		testid?: string;
	} = $props();

	let term = $state('');
	let results = $state<ShipSearchResult[]>([]);
	const search = latest(api.searchShips, (found) => (results = found));
	let searchTimer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => {
		const text = term.trim();
		clearTimeout(searchTimer);
		if (!text) {
			results = [];
			return;
		}
		searchTimer = setTimeout(() => search(text), 200);
	});
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
