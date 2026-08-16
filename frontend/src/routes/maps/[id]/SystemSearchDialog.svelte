<script lang="ts">
	// A command-palette picker for adding a solar system to a map. Results come from the
	// server as you type, so the Command's built-in filtering is disabled.
	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import * as Command from '$lib/components/ui/command';
	import { searchClassification } from '$lib/map/classes';

	let {
		open = $bindable(),
		onpick
	}: {
		open: boolean;
		onpick: (solarSystemId: number) => void;
	} = $props();

	let query = $state('');
	let results = $state<SystemSearchResult[]>([]);
	// Monotonic request id: drop responses that arrive out of order while typing.
	let generation = 0;

	// Reset each time the dialog opens.
	$effect(() => {
		if (open) {
			query = '';
			results = [];
		}
	});

	$effect(() => {
		const text = query;
		const request = ++generation;
		api
			.searchSystems(text)
			.then((found) => {
				if (generation === request) results = found;
			})
			.catch(() => {});
	});

	function choose(id: number) {
		onpick(id);
		open = false;
	}
</script>

<Command.Dialog
	bind:open
	shouldFilter={false}
	title="Search systems"
	description="Search the universe for a solar system to add."
>
	<Command.Input placeholder="Search for a system…" bind:value={query} />
	<Command.List>
		<Command.Empty>
			{query.trim().length < 2 ? 'Type at least two characters to search.' : 'No systems found.'}
		</Command.Empty>
		<Command.Group>
			{#each results as s (s.id)}
				{@const c = searchClassification(s)}
				<Command.Item value={String(s.id)} onSelect={() => choose(s.id)}>
					<span class="w-12 shrink-0 font-mono text-xs" style="color: var(--color-{c.token})">{c.badge}</span>
					<span class="truncate text-foreground">{s.name}</span>
					<span class="ml-auto truncate text-xs text-muted-foreground">{s.region}</span>
				</Command.Item>
			{/each}
		</Command.Group>
	</Command.List>
</Command.Dialog>
