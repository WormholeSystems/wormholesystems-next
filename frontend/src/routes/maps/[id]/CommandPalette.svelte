<script lang="ts">
	// Cmd+K over one map: jump to a placed system, or add one that is not on the map yet.
	// Matching happens server-side (name, alias, occupier, and notes for members), so the
	// Command's own filtering is off and the rows arrive already ranked.
	//
	// Rows are the shared SystemRow, on the shared tracks, so the palette lines up with every
	// other system list. The extra cells (why it matched, the Add badge) are tracks appended
	// around it rather than a hand-rolled layout.
	import PlusIcon from '@lucide/svelte/icons/plus';

	import { api } from '$lib/api/client';
	import type { MapSearchHit } from '$lib/api/types/MapSearchHit';
	import { Badge } from '$lib/components/ui/badge';
	import * as Command from '$lib/components/ui/command';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import { NODE_W, centerWorld, freePosition } from '$lib/map/helpers';
	import type { MapState } from './map-state.svelte';

	let { map, open = $bindable() }: { map: MapState; open: boolean } = $props();

	let query = $state('');
	let results = $state<MapSearchHit[]>([]);
	// Monotonic request id: drop responses that arrive out of order while typing.
	let generation = 0;

	const canWrite = $derived(map.data?.role === 'member' || map.data?.role === 'owner');
	const onMap = $derived(results.filter((h) => h.map_solar_system_id !== null));
	const offMap = $derived(results.filter((h) => h.map_solar_system_id === null));

	// The four SystemRow tracks, with one appended for the match hint or the Add badge.
	const TRACKS =
		'grid w-full grid-cols-[min-content_minmax(0,1fr)_minmax(0,0.8fr)_min-content_min-content] items-center gap-x-2';

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

	/** What to show in the trailing cell: the reason this row matched, when it was not the name. */
	function hint(h: MapSearchHit): string | null {
		if (h.note_excerpt) return h.note_excerpt;
		if (h.matched === 'alias') return h.alias;
		if (h.matched === 'occupier') return h.occupying_group;
		return null;
	}

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
				solar_system_id: hit.system.id,
				x: at.x,
				y: at.y,
				alias: null
			})
		);
	}
</script>

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
						value={`on-${hit.system.id}`}
						onSelect={() => activate(hit)}
						data-testid="palette-hit"
					>
						<SystemMenu system={hit.system} class={TRACKS}>
							<SystemRow system={hit.system} />
							<span class="truncate text-xs text-muted-foreground" title={hint(hit) ?? undefined}>
								{hint(hit) ?? ''}
							</span>
						</SystemMenu>
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
		{#if canWrite && offMap.length > 0}
			<Command.Group heading="Add to the map">
				{#each offMap as hit (hit.system.id)}
					<Command.Item
						value={`off-${hit.system.id}`}
						onSelect={() => add(hit)}
						data-testid="palette-add"
					>
						<SystemMenu system={hit.system} class={TRACKS}>
							<SystemRow system={hit.system} />
							<Badge variant="outline" class="gap-1 shrink-0">
								<PlusIcon />
								Add
							</Badge>
						</SystemMenu>
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
	</Command.List>
</Command.Dialog>
