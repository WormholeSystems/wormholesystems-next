<script lang="ts">
	// Cmd+K over one map: jump to a placed system, or add one that is not on the map yet.
	//
	// Also the map's only way in to a system search: right-clicking the canvas or a node and
	// asking to add or connect opens this, with an anchor for where the result should land
	// and, for a connection, the node it should hang off. One search UI rather than two that
	// drifted apart, and the palette already knew how to do both halves.
	// Matching happens server-side (name, alias, occupier, and notes for members), so the
	// Command's own filtering is off and the rows arrive already ranked.
	//
	// Rows are the shared SystemRow, on the shared tracks, so the palette lines up with every
	// other system list. The extra cell (why it matched, or the Add badge) is a track
	// appended to those rather than a hand-rolled layout.
	import PlusIcon from '@lucide/svelte/icons/plus';

	import { api } from '$lib/api/client';
	import type { MapSearchHit } from '$lib/api/types/MapSearchHit';
	import { Badge } from '$lib/components/ui/badge';
	import * as Command from '$lib/components/ui/command';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import { NODE_W, centerWorld, freePosition, heuristicSize } from '$lib/map/helpers';
	import type { MapState } from './map-state.svelte';

	let { map, open = $bindable() }: { map: MapState; open: boolean } = $props();

	let query = $state('');
	let results = $state<MapSearchHit[]>([]);
	// Monotonic request id: drop responses that arrive out of order while typing.
	let generation = 0;

	const canWrite = $derived(map.data?.role === 'member' || map.data?.role === 'owner');
	// Threat hits are systems too, but they answer a different question — where does this
	// corp operate — so they get their own section rather than being mixed into results
	// that matched by name.
	const threats = $derived(results.filter((h) => h.threat));
	/**
	 * One section per organisation, the way legacy's threat search reads.
	 *
	 * Flat rows would repeat the name you just typed on every line and squeeze the kill
	 * count — the one number that differs between them — out of the cell.
	 */
	const threatGroups = $derived.by(() => {
		const groups = new Map<number, { name: string; kind: string; total: number; hits: MapSearchHit[] }>();
		for (const hit of threats) {
			const t = hit.threat!;
			const group = groups.get(t.entity_id) ?? { name: t.name, kind: t.entity_type, total: 0, hits: [] };
			group.total += t.kills;
			group.hits.push(hit);
			groups.set(t.entity_id, group);
		}
		return [...groups.entries()].map(([id, group]) => ({ id, ...group }));
	});
	const named = $derived(results.filter((h) => !h.threat));
	/** Opened from "connect to a new system": every pick becomes a connection as well. */
	const linking = $derived(map.linkFrom !== null);
	const onMap = $derived(named.filter((h) => h.map_solar_system_id !== null));
	const offMap = $derived(named.filter((h) => h.map_solar_system_id === null));

	// The list owns the tracks; rows are subgrids of them. Both groups sit in the one grid,
	// so an on-map row and an off-map row line up with each other, not just within a group.
	// Tracks: the four SystemRow cells, the hint/badge, and Command's own check indicator.
	const LIST_TRACKS =
		'grid grid-cols-[min-content_minmax(0,1fr)_minmax(0,0.8fr)_min-content_minmax(0,0.7fr)_min-content] items-center gap-x-2';
	const ROW = 'col-span-full grid grid-cols-subgrid items-center gap-x-2';
	const CELLS = 'col-span-5 grid grid-cols-subgrid items-center gap-x-2';
	const HEADING = 'col-span-full px-2 pt-2 pb-1 text-xs font-medium text-muted-foreground';

	$effect(() => {
		if (open) {
			query = '';
			results = [];
		} else {
			// Closing without picking drops the pending placement, so the next Cmd+K is a
			// plain search again rather than quietly still linking.
			map.linkFrom = null;
			map.searchAnchor = null;
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
		// Already on the map, and we were asked for a connection: join the two instead of
		// panning to it.
		if (linking && hit.map_solar_system_id !== null) {
			connect(hit.map_solar_system_id);
			return;
		}
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

	/** Join an already-placed system to whatever asked for the connection. */
	function connect(target: number) {
		const from = map.linkFrom;
		open = false;
		if (from === null || from === target) return;
		map.run(
			'connect',
			api.addConnection({
				map_id: map.mapId,
				from_system: from,
				to_system: target,
				kind: 'wormhole',
				size: heuristicSize(map.systems, from, target)
			})
		);
	}

	/** Jump to it if it is already placed, otherwise put it on the map. */
	function open_(hit: MapSearchHit) {
		if (hit.map_solar_system_id !== null) activate(hit);
		else if (canWrite) add(hit);
	}

	function add(hit: MapSearchHit) {
		const from = map.linkFrom;
		// Where the caller asked for it: the point the map was right-clicked, or the node the
		// connection starts from. Plain Cmd+K has no anchor, so it lands in the middle of
		// what you are looking at.
		const base = map.searchAnchor ?? centerWorld(map.pan, map.zoom, map.viewportRect());
		open = false;
		const at = freePosition(map.systems, base, map.grid);
		map.run(
			'add',
			(async () => {
				const placed = await api.addSystem({
					map_id: map.mapId,
					solar_system_id: hit.system.id,
					x: at.x,
					y: at.y,
					alias: null
				});
				if (from !== null && from !== placed.id) {
					await api.addConnection({
						map_id: map.mapId,
						from_system: from,
						to_system: placed.id,
						kind: 'wormhole',
						size: heuristicSize(map.systems, from, placed.id)
					});
				}
			})()
		);
	}
</script>

<Command.Dialog
	bind:open
	shouldFilter={false}
	title={linking ? 'Connect to a system' : 'Search this map'}
	description={linking
		? 'Pick the system on the other side of the connection.'
		: 'Jump to a system on the map, or add one that is not on it yet.'}
>
	<Command.Input
		placeholder={linking ? 'Connect to…' : 'System, alias, occupier or notes…'}
		bind:value={query}
	/>
	<!-- Headings are plain cells rather than Command.Group, because a group wraps its items
	     in an element of its own and that break in the ancestry is exactly what a subgrid
	     cannot cross. -->
	<Command.List class="p-1 {LIST_TRACKS}" data-testid="palette-list">
		<Command.Empty class="col-span-full">
			{query.trim().length < 2 ? 'Type at least two characters to search.' : 'Nothing found.'}
		</Command.Empty>
		{#if onMap.length > 0}
			<div class={HEADING}>On this map</div>
			{#each onMap as hit (hit.map_solar_system_id)}
				<Command.Item
					value={`on-${hit.system.id}`}
					onSelect={() => activate(hit)}
					class={ROW}
					data-testid="palette-hit"
				>
					<SystemMenu system={hit.system} class={CELLS}>
						<SystemRow system={hit.system} />
						<span class="truncate text-xs text-muted-foreground" title={hint(hit) ?? undefined}>
							{hint(hit) ?? ''}
						</span>
					</SystemMenu>
				</Command.Item>
			{/each}
		{/if}
		{#each threatGroups as group (group.id)}
			<div class="{HEADING} flex items-center gap-2" data-testid="palette-threat-group">
				<span class="min-w-0 truncate text-foreground" title="{group.name} ({group.kind})">
					{group.name}
				</span>
				<span class="ml-auto shrink-0 font-mono whitespace-nowrap tabular-nums text-muted-foreground/60">
					{group.hits.length} × {group.total.toLocaleString()} kills
				</span>
			</div>
			{#each group.hits as hit (hit.system.id)}
				<Command.Item
					value={`threat-${group.id}-${hit.system.id}`}
					onSelect={() => open_(hit)}
					class={ROW}
					data-testid="palette-threat"
				>
					<SystemMenu system={hit.system} class={CELLS}>
						<SystemRow system={hit.system} />
						<span
							class="text-right font-mono text-xs tabular-nums text-muted-foreground"
							data-testid="palette-threat-kills"
						>
							{hit.threat?.kills.toLocaleString()}
						</span>
					</SystemMenu>
				</Command.Item>
			{/each}
		{/each}
		{#if canWrite && offMap.length > 0}
			<div class={HEADING}>{linking ? 'Add and connect' : 'Add to the map'}</div>
			{#each offMap as hit (hit.system.id)}
				<Command.Item
					value={`off-${hit.system.id}`}
					onSelect={() => add(hit)}
					class={ROW}
					data-testid="palette-add"
				>
					<SystemMenu system={hit.system} class={CELLS}>
						<SystemRow system={hit.system} />
						<Badge variant="outline" class="justify-self-end gap-1">
							<PlusIcon />
							Add
						</Badge>
					</SystemMenu>
				</Command.Item>
			{/each}
		{/if}
	</Command.List>
</Command.Dialog>
