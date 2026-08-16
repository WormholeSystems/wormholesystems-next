<script lang="ts">
	// Cmd+K over one map: jump to a placed system, or add one that is not on the map yet.
	// Matching happens server-side (name, alias, occupier, and notes for members), so the
	// Command's own filtering is off and the rows arrive already ranked.
	import PlusIcon from '@lucide/svelte/icons/plus';

	import { api } from '$lib/api/client';
	import type { MapSearchHit } from '$lib/api/types/MapSearchHit';
	import { Badge } from '$lib/components/ui/badge';
	import * as Command from '$lib/components/ui/command';
	import { classMeta } from '$lib/map/classes';
	import { NODE_W, centerWorld, freePosition } from '$lib/map/helpers';
	import { cn } from '$lib/utils';
	import type { MapState } from './map-state.svelte';

	let { map, open = $bindable() }: { map: MapState; open: boolean } = $props();

	let query = $state('');
	let results = $state<MapSearchHit[]>([]);
	// Monotonic request id: drop responses that arrive out of order while typing.
	let generation = 0;

	const canWrite = $derived(map.data?.role === 'member' || map.data?.role === 'owner');
	const onMap = $derived(results.filter((h) => h.map_solar_system_id !== null));
	const offMap = $derived(results.filter((h) => h.map_solar_system_id === null));

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
			.searchMap(map.mapId, text)
			.then((found) => {
				if (generation === request) results = found;
			})
			.catch(() => {});
	});

	function activate(hit: MapSearchHit) {
		map.activeId = hit.map_solar_system_id;
		open = false;
		// Pan the node into the middle, so a jump from the palette actually shows it.
		const system = map.systems.find((s) => s.id === hit.map_solar_system_id);
		if (!system) return;
		const r = map.viewportRect();
		map.pan = {
			x: r.width / 2 - (system.position_x + NODE_W / 2) * map.zoom,
			y: r.height / 2 - (system.position_y + map.nodeH / 2) * map.zoom
		};
	}

	function add(hit: MapSearchHit) {
		open = false;
		const base = centerWorld(map.pan, map.zoom, map.viewportRect());
		const at = freePosition(map.systems, base, map.grid);
		map.run(
			'add',
			api.addSystem({
				map_id: map.mapId,
				solar_system_id: hit.solar_system_id,
				x: at.x,
				y: at.y,
				alias: null
			})
		);
	}
</script>

{#snippet row(hit: MapSearchHit)}
	{@const meta = classMeta(hit.wormhole_class_id, hit.security)}
	<div class="flex w-full items-center gap-2">
		<span class={cn('w-8 shrink-0 font-mono text-xs', meta.token)}>{meta.short}</span>
		<span class="shrink-0">{hit.name}</span>
		{#if hit.alias}
			<span class="shrink-0 text-xs text-muted-foreground">{hit.alias}</span>
		{/if}
		<span class="flex-1 truncate text-xs text-muted-foreground">
			{#if hit.note_excerpt}
				{hit.note_excerpt}
			{:else if hit.matched === 'occupier' && hit.occupying_group}
				{hit.occupying_group}
			{:else}
				{hit.region}
			{/if}
		</span>
	</div>
{/snippet}

<Command.Dialog
	bind:open
	shouldFilter={false}
	title="Search this map"
	description="Jump to a system on the map, or add one that is not on it yet."
>
	<Command.Input placeholder="System, alias, occupier or notes…" bind:value={query} />
	<Command.List data-testid="palette-list">
		<Command.Empty>
			{query.trim().length < 2 ? 'Type at least two characters to search.' : 'Nothing found.'}
		</Command.Empty>
		{#if onMap.length > 0}
			<Command.Group heading="On this map">
				{#each onMap as hit (hit.map_solar_system_id)}
					<Command.Item
						value={`on-${hit.solar_system_id}`}
						onSelect={() => activate(hit)}
						data-testid="palette-hit"
					>
						{@render row(hit)}
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
		{#if canWrite && offMap.length > 0}
			<Command.Group heading="Add to the map">
				{#each offMap as hit (hit.solar_system_id)}
					<Command.Item
						value={`off-${hit.solar_system_id}`}
						onSelect={() => add(hit)}
						data-testid="palette-add"
					>
						{@render row(hit)}
						<Badge variant="outline" class="ml-auto gap-1 shrink-0">
							<PlusIcon />
							Add
						</Badge>
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
	</Command.List>
</Command.Dialog>
