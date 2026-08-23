<script lang="ts">
	// The header's "+": search any system and put it on the shared watchlist.
	import PlusIcon from '@lucide/svelte/icons/plus';

	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import { latest } from '$lib/latest';
	import type { MapState } from '../map-state.svelte';

	let { map }: { map: MapState } = $props();

	let addOpen = $state(false);
	let addQuery = $state('');
	let addResults = $state<SystemSearchResult[]>([]);
	const addSearch = latest(api.searchSystems, (found) => (addResults = found));
	$effect(() => {
		const text = addQuery.trim();
		if (!text) {
			addResults = [];
			return;
		}
		addSearch(text);
	});
	function addToWatchlist(id: number) {
		map.run('watch', api.addWatchlistEntry({ map_id: map.mapId, solar_system_id: id }));
		addOpen = false;
	}
</script>

<Popover.Root bind:open={addOpen}>
	<Popover.Trigger
		class="text-muted-foreground transition-colors hover:text-foreground"
		title="Add to watchlist"
		aria-label="Add to watchlist"
		data-testid="watchlist-add"
	>
		<PlusIcon class="size-4" />
	</Popover.Trigger>
	<Popover.Content class="w-96 p-0" align="end">
		<Command.Root shouldFilter={false}>
			<Command.Input placeholder="Watch a system…" bind:value={addQuery} />
			<!-- The list owns the tracks and rows are subgrids of them. The trailing
			     track is Command's own check indicator, appended to every item. -->
			<Command.List
				class="max-h-48 grid grid-cols-[min-content_minmax(0,1fr)_minmax(0,0.8fr)_min-content_min-content] items-center gap-x-2 p-1"
			>
				<Command.Empty class="col-span-full">
					{addQuery.trim().length < 2
						? 'Type at least two characters to search.'
						: 'No systems found.'}
				</Command.Empty>
				{#each addResults as s (s.id)}
					<Command.Item
						value={String(s.id)}
						onSelect={() => addToWatchlist(s.id)}
						class="col-span-full grid grid-cols-subgrid items-center gap-x-2"
					>
						<SystemMenu system={s} class="col-span-4 grid grid-cols-subgrid items-center gap-x-2">
							<SystemRow system={s} />
						</SystemMenu>
					</Command.Item>
				{/each}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
