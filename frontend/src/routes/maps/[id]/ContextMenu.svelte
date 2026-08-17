<script lang="ts">
	// The right-click menu, structured and iconed to legacy parity. Hand-rolled
	// (coordinate-positioned) with CSS-hover flyout submenus.
	import CheckIcon from '@lucide/svelte/icons/check';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import ClockIcon from '@lucide/svelte/icons/clock';
	import CompassIcon from '@lucide/svelte/icons/compass';
	import EraserIcon from '@lucide/svelte/icons/eraser';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
	import FlagIcon from '@lucide/svelte/icons/flag';
	import GlobeIcon from '@lucide/svelte/icons/globe';
	import HomeIcon from '@lucide/svelte/icons/home';
	import MapIcon from '@lucide/svelte/icons/map';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import PinIcon from '@lucide/svelte/icons/pin';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import RouteIcon from '@lucide/svelte/icons/route';
	import ShipIcon from '@lucide/svelte/icons/ship';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import UsersIcon from '@lucide/svelte/icons/users';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import WeightIcon from '@lucide/svelte/icons/weight';

	import { api } from '$lib/api/client';
	import type { ConnectionType } from '$lib/api/types/ConnectionType';
	import type { MassStatus } from '$lib/api/types/MassStatus';
	import type { SystemStatus } from '$lib/api/types/SystemStatus';
	import type { TimeStatus } from '$lib/api/types/TimeStatus';
	import type { WormholeSize } from '$lib/api/types/WormholeSize';
	import EveImage from '$lib/components/EveImage.svelte';
	import { isWormholeClass } from '$lib/map/classes';
	import { NODE_W, statusColor } from '$lib/map/helpers';
	import { STATUS_ICONS, STATUS_OPTIONS, statusLabel } from '$lib/map/status';
	import type { MapState, Menu } from './map-state.svelte';

	let { map, menu }: { map: MapState; menu: Menu } = $props();

	const item =
		'flex w-full items-center gap-2 px-3 py-1 text-left text-xs text-foreground hover:bg-accent';
	const sub =
		'relative group/sub flex w-full cursor-default items-center gap-2 px-3 py-1 text-left text-xs text-foreground hover:bg-accent';
	const panel =
		'absolute left-full top-0 z-40 hidden min-w-40 border border-border bg-popover py-1 shadow-md group-hover/sub:block';

	// Legacy ship-size catalogue (vector's `small` is legacy `frigate`).
	const SIZE_OPTIONS: { value: WormholeSize; label: string; letter: string }[] = [
		{ value: 'small', label: 'Frigate', letter: 'S' },
		{ value: 'medium', label: 'Medium', letter: 'M' },
		{ value: 'large', label: 'Large', letter: 'L' },
		{ value: 'xl', label: 'Extra Large', letter: 'XL' }
	];

	function close() {
		map.closeMenu();
	}

	// --- map ---

	function addSystem() {
		map.linkFrom = null;
		// Land the new system where the map was right-clicked (centered on the click).
		const w = map.toWorld(menu.x, menu.y);
		map.searchAnchor = { x: w.x - NODE_W / 2, y: w.y - map.nodeH / 2 };
		map.searchOpen = true;
		close();
	}

	function deleteSelection() {
		const ids = [...map.selected];
		map.selected = new Set();
		map.run('remove', api.removeSystems({ map_id: map.mapId, map_solar_system_ids: ids }));
		close();
	}

	function clearMap() {
		if (confirm('Clear the map? This removes all systems except home and pinned ones.')) {
			map.run('clear map', api.clearMap({ map_id: map.mapId }));
		}
		close();
	}

	// --- node ---

	function connectFrom(id: number) {
		map.linkFrom = id;
		// Anchor on the source node itself; the placement helper steps out from there and
		// owns the spacing, so every way of adding a system leaves the same gap.
		const s = map.systems.find((s) => s.id === id);
		map.searchAnchor = s ? { x: s.position_x, y: s.position_y } : null;
		map.searchOpen = true;
		close();
	}

	function setStatus(id: number, status: SystemStatus) {
		map.run('status', api.setStatus({ map_id: map.mapId, map_solar_system_id: id, status }));
		close();
	}

	function togglePin(id: number, value: boolean) {
		map.run('pin', api.setPinned({ map_id: map.mapId, map_solar_system_id: id, value }));
		close();
	}

	function toggleHome(id: number, value: boolean) {
		map.run('home', api.setHome({ map_id: map.mapId, map_solar_system_id: id, value }));
		close();
	}

	function toggleRally(id: number, value: boolean) {
		map.run('rally', api.setRally({ map_id: map.mapId, map_solar_system_id: id, value }));
		close();
	}

	/** Removes the whole marquee selection when one exists, else just this node. */
	function removeSystem(id: number) {
		const ids = map.selected.size > 0 ? [...map.selected] : [id];
		map.selected = new Set();
		map.run('remove', api.removeSystems({ map_id: map.mapId, map_solar_system_ids: ids }));
		close();
	}

	// --- ESI waypoints (k-space targets only) ---

	const onlineCharacters = $derived(map.myCharacters.filter((c) => c.online));

	function waypoint(characterId: number, destinationId: number, clearOthers: boolean) {
		map.run(
			'waypoint',
			api.setWaypoint({
				character_id: characterId,
				destination_id: destinationId,
				clear_other_waypoints: clearOthers
			})
		);
		close();
	}

	function waypointAll(destinationId: number, clearOthers: boolean) {
		map.run(
			'waypoint',
			api.setWaypointAll({ destination_id: destinationId, clear_other_waypoints: clearOthers })
		);
		close();
	}

	// --- connection ---

	const connection = $derived(
		menu.target.kind === 'connection'
			? (map.connections.find((c) => c.id === (menu.target as { id: number }).id) ?? null)
			: null
	);

	function setKind(cid: number, kind: ConnectionType) {
		map.run('conn type', api.setConnectionStatus({ map_id: map.mapId, connection_id: cid, kind }));
		close();
	}

	function setMass(cid: number, mass: MassStatus) {
		map.run(
			'conn mass',
			api.setConnectionStatus({ map_id: map.mapId, connection_id: cid, mass_status: mass })
		);
		close();
	}

	function setLifetime(cid: number, time: TimeStatus) {
		map.run(
			'conn lifetime',
			api.setConnectionStatus({ map_id: map.mapId, connection_id: cid, time_status: time })
		);
		close();
	}

	function setSize(cid: number, size: WormholeSize) {
		map.run(
			'conn size',
			api.setConnectionStatus({ map_id: map.mapId, connection_id: cid, size })
		);
		close();
	}

	function removeConnection(cid: number) {
		map.run('del conn', api.removeConnection({ map_id: map.mapId, connection_id: cid }));
		close();
	}

	const underscore = (s: string) => s.replaceAll(' ', '_');
</script>

{#snippet waypointSubmenu(label: string, destinationId: number, clearOthers: boolean)}
	{@const LabelIcon = clearOthers ? NavigationIcon : MapPinIcon}
	<div class={sub} data-testid="{clearOthers ? 'destination' : 'waypoint'}-subtrigger">
		<LabelIcon class="size-4" />
		{label}
		<ChevronRightIcon class="ml-auto size-3" />
		<div class={panel} data-testid="{clearOthers ? 'destination' : 'waypoint'}-submenu">
			{#if onlineCharacters.length === 0}
				<div class="px-3 py-1 text-xs text-muted-foreground">No characters online</div>
			{:else}
				{#each onlineCharacters as c (c.character_id)}
					<button class={item} onclick={() => waypoint(c.character_id, destinationId, clearOthers)}>
						<EveImage kind="character" id={c.character_id} class="size-5 rounded-lg" />
						{c.name}
					</button>
				{/each}
				{#if onlineCharacters.length > 1}
					<div class="my-0.5 border-t border-border"></div>
					<button class={item} onclick={() => waypointAll(destinationId, clearOthers)}>
						<UsersIcon class="size-4" />
						All Characters
					</button>
				{/if}
			{/if}
		</div>
	</div>
{/snippet}

{#snippet dot(color: string)}
	<span class="inline-block size-2 shrink-0 rounded-full" style="background-color: {color}"></span>
{/snippet}

{#snippet check(selected: boolean)}
	{#if selected}
		<CheckIcon class="size-3.5 shrink-0" />
	{/if}
{/snippet}

<!-- Keep pointerdown from reaching the canvas: its background handler closes the menu,
which would unmount these buttons before their click can fire. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed z-30 min-w-44 border border-border bg-popover py-1 shadow-md"
	data-testid="context-menu"
	style:left="{menu.x}px"
	style:top="{menu.y}px"
	onpointerdown={(ev) => ev.stopPropagation()}
	oncontextmenu={(ev) => {
		ev.preventDefault();
		ev.stopPropagation();
	}}
>
	{#if menu.target.kind === 'map'}
		<button class={item} onclick={addSystem}>
			<PlusIcon class="size-4" />
			Add solar system
		</button>
		{#if map.selected.size > 0}
			<button class={item} onclick={deleteSelection}>
				<Trash2Icon class="size-4" />
				Delete selection
			</button>
		{/if}
		<button class={item} onclick={clearMap}>
			<EraserIcon class="size-4" />
			Clear map
		</button>
	{:else if menu.target.kind === 'node'}
		{@const s = menu.target.system}
		<button class={item} onclick={() => togglePin(s.id, !s.is_pinned)}>
			<PinIcon class="size-4" />
			{s.is_pinned ? 'Unpin' : 'Pin'}
		</button>
		<button class={item} onclick={() => connectFrom(s.id)}>
			<WaypointsIcon class="size-4" />
			Add connection
		</button>

		<div class={sub} data-testid="status-subtrigger">
			<MapIcon class="size-4" />
			Status
			<ChevronRightIcon class="ml-auto size-3" />
			<div class={panel} data-testid="status-submenu">
				{#each STATUS_OPTIONS as st (st)}
					{@const Icon = STATUS_ICONS[st]}
					<button class={item} onclick={() => setStatus(s.id, st)}>
						{statusLabel(st)}
						<Icon class="ml-auto size-3.5" style="color: {statusColor(st)}" />
					</button>
				{/each}
			</div>
		</div>

		<div class="my-0.5 border-t border-border"></div>

		<div class={sub} data-testid="external-subtrigger">
			<ExternalLinkIcon class="size-4" />
			External
			<ChevronRightIcon class="ml-auto size-3" />
			<div class={panel} data-testid="external-submenu">
				<div
					class="px-3 py-1 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase"
				>
					Dotlan
				</div>
				<a
					class={item}
					href="https://evemaps.dotlan.net/system/{underscore(s.name)}"
					target="_blank"
					rel="noopener"
				>
					<GlobeIcon class="size-4" /> System
				</a>
				<a
					class={item}
					href="https://evemaps.dotlan.net/map/{underscore(s.region)}/{underscore(s.name)}"
					target="_blank"
					rel="noopener"
				>
					<MapIcon class="size-4" /> Region Map
				</a>
				{#if !isWormholeClass(s.wormhole_class_id)}
					<a
						class={item}
						href="https://evemaps.dotlan.net/range/Revelation,5/{underscore(s.name)}"
						target="_blank"
						rel="noopener"
					>
						<CompassIcon class="size-4" /> Jump Range
					</a>
				{/if}
				<div class="my-0.5 border-t border-border"></div>
				<div
					class="px-3 py-1 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase"
				>
					zKillboard
				</div>
				<a
					class={item}
					href="https://zkillboard.com/system/{s.solar_system_id}/"
					target="_blank"
					rel="noopener"
				>
					<GlobeIcon class="size-4" /> System
				</a>
				<a
					class={item}
					href="https://zkillboard.com/constellation/{s.constellation_id}/"
					target="_blank"
					rel="noopener"
				>
					<CompassIcon class="size-4" /> Constellation
				</a>
				<a
					class={item}
					href="https://zkillboard.com/region/{s.region_id}/"
					target="_blank"
					rel="noopener"
				>
					<MapIcon class="size-4" /> Region
				</a>
			</div>
		</div>

		{#if !isWormholeClass(s.wormhole_class_id)}
			{@render waypointSubmenu('Set destination', s.solar_system_id, true)}
			{@render waypointSubmenu('Add waypoint', s.solar_system_id, false)}
		{/if}

		<div class={sub} data-testid="route-subtrigger">
			<RouteIcon class="size-4" />
			Route planner
			<ChevronRightIcon class="ml-auto size-3" />
			<div class={panel} data-testid="route-submenu">
				<button
					class={item}
					onclick={() => {
						map.routeFromId = s.solar_system_id;
						close();
					}}
				>
					<CompassIcon class="size-4" />
					Set as origin
				</button>
				<button
					class={item}
					onclick={() => {
						map.routeToId = s.solar_system_id;
						close();
					}}
				>
					<NavigationIcon class="size-4" />
					Set as destination
				</button>
			</div>
		</div>

		<div class="my-0.5 border-t border-border"></div>

		<button class={item} onclick={() => toggleHome(s.id, !s.is_home)}>
			<HomeIcon class="size-4" />
			{s.is_home ? 'Unset Home System' : 'Set as Home System'}
		</button>
		<button class={item} onclick={() => toggleRally(s.id, !s.is_rally)}>
			<FlagIcon class="size-4" />
			{s.is_rally ? 'Clear Rally Point' : 'Set as Rally Point'}
		</button>

		{#if !s.is_pinned && !s.is_home}
			<div class="my-0.5 border-t border-border"></div>
			<button
				class="{item} text-destructive hover:text-destructive"
				onclick={() => removeSystem(s.id)}
			>
				<Trash2Icon class="size-4" />
				Remove
			</button>
		{/if}
	{:else if connection}
		{@const cid = connection.id}
		<div class={sub} data-testid="lifetime-subtrigger">
			<ClockIcon class="size-4" />
			Lifetime
			<ChevronRightIcon class="ml-auto size-3" />
			<div class={panel} data-testid="lifetime-submenu">
				<button class={item} onclick={() => setLifetime(cid, 'stable')}>
					{@render dot('oklch(55.6% 0.007 260)')}
					Healthy
					<span class="ml-auto"></span>
					{@render check(connection.time_status === 'stable' || connection.time_status === null)}
				</button>
				<button class={item} onclick={() => setLifetime(cid, 'eol')}>
					{@render dot('#a855f7')}
					End of Life
					<span class="ml-auto text-muted-foreground">&lt; 4h</span>
					{@render check(connection.time_status === 'eol')}
				</button>
				<button class={item} onclick={() => setLifetime(cid, 'critical')}>
					{@render dot('#ef4444')}
					Critical
					<span class="ml-auto text-muted-foreground">&lt; 1h</span>
					{@render check(connection.time_status === 'critical')}
				</button>
			</div>
		</div>

		<div class={sub} data-testid="mass-subtrigger">
			<WeightIcon class="size-4" />
			Mass Status
			<ChevronRightIcon class="ml-auto size-3" />
			<div class={panel} data-testid="mass-submenu">
				<button class={item} onclick={() => setMass(cid, 'stable')}>
					{@render dot('oklch(55.6% 0.007 260)')}
					Fresh
					<span class="ml-auto text-muted-foreground">&ge; 50%</span>
					{@render check(connection.mass_status === 'stable' || connection.mass_status === null)}
				</button>
				<button class={item} onclick={() => setMass(cid, 'reduced')}>
					{@render dot('#f59e0b')}
					Reduced
					<span class="ml-auto text-muted-foreground">&lt; 50%</span>
					{@render check(connection.mass_status === 'reduced')}
				</button>
				<button class={item} onclick={() => setMass(cid, 'critical')}>
					{@render dot('#ef4444')}
					Critical
					<span class="ml-auto text-muted-foreground">&le; 15%</span>
					{@render check(connection.mass_status === 'critical')}
				</button>
			</div>
		</div>

		<div class={sub} data-testid="size-subtrigger">
			<ShipIcon class="size-4" />
			Ship Size
			<ChevronRightIcon class="ml-auto size-3" />
			<div class={panel} data-testid="size-submenu">
				{#each SIZE_OPTIONS as o (o.value)}
					<button class={item} onclick={() => setSize(cid, o.value)}>
						<span class="inline-flex w-6 justify-center font-mono text-[10px] text-muted-foreground">
							{o.letter}
						</span>
						{o.label}
						<span class="ml-auto"></span>
						{@render check(connection.size === o.value)}
					</button>
				{/each}
			</div>
		</div>

		<div class={sub} data-testid="type-subtrigger">
			<WaypointsIcon class="size-4" />
			Connection type
			<ChevronRightIcon class="ml-auto size-3" />
			<div class={panel} data-testid="type-submenu">
				<button class={item} onclick={() => setKind(cid, 'wormhole')}>
					Wormhole
					<span class="ml-auto"></span>
					{@render check(connection.kind === 'wormhole')}
				</button>
				<button class={item} onclick={() => setKind(cid, 'stargate')}>
					Stargate
					{#if connection.kind !== 'stargate'}
						<TriangleAlertIcon class="ml-auto size-3.5 text-amber-500" />
					{:else}
						<span class="ml-auto"></span>
						{@render check(true)}
					{/if}
				</button>
			</div>
		</div>

		<div class="my-0.5 border-t border-border"></div>
		<button
			class="{item} text-destructive hover:text-destructive"
			onclick={() => removeConnection(cid)}
		>
			<Trash2Icon class="size-4" />
			Remove
		</button>
	{/if}
</div>
