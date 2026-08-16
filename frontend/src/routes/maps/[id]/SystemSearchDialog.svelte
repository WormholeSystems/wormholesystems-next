<script lang="ts">
	// A command-palette picker for adding a solar system to a map. Results come from the
	// server as you type, so the Command's built-in filtering is disabled.
	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import * as Command from '$lib/components/ui/command';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';

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
				<Command.Item value={String(s.id)} onSelect={() => choose(s.id)}>
					<SystemMenu system={s} class="grid w-full grid-cols-[min-content_minmax(0,1fr)_minmax(0,0.8fr)_min-content] items-center gap-x-2">
						<SystemRow system={s} />
					</SystemMenu>
				</Command.Item>
			{/each}
		</Command.Group>
	</Command.List>
</Command.Dialog>
