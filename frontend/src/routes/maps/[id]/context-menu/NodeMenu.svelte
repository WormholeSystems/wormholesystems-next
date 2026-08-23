<script lang="ts">
	// Right-clicking a system node: pin, connect, status, external sites, waypoints, and the
	// home/rally flags.
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import CompassIcon from '@lucide/svelte/icons/compass';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
	import FlagIcon from '@lucide/svelte/icons/flag';
	import GlobeIcon from '@lucide/svelte/icons/globe';
	import HomeIcon from '@lucide/svelte/icons/home';
	import MapIcon from '@lucide/svelte/icons/map';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import PinIcon from '@lucide/svelte/icons/pin';
	import RouteIcon from '@lucide/svelte/icons/route';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import UsersIcon from '@lucide/svelte/icons/users';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';

	import { api } from '$lib/api/client';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { SystemStatus } from '$lib/api/types/SystemStatus';
	import EveImage from '$lib/components/EveImage.svelte';
	import { isWormholeClass } from '$lib/map/classes';
	import {
		dotlanJumpRangeUrl,
		dotlanRegionMapUrl,
		dotlanSystemUrl,
		zkillboardConstellationUrl,
		zkillboardRegionUrl,
		zkillboardSystemUrl,
	} from '$lib/map/external-links';
	import { statusColor } from '$lib/map/helpers';
	import { STATUS_ICONS, STATUS_OPTIONS, statusLabel } from '$lib/map/status';
	import type { MapState } from '../map-state.svelte';
	import { item, panel, sub } from './chrome';

	let { map, system: s }: { map: MapState; system: MapSystemView } = $props();

	function close() {
		map.closeMenu();
	}

	/** The palette doubles as the picker: on-map hits merge, off-map ones name the ghost. */
	function assignSystem(id: number) {
		map.assignGhostId = id;
		map.linkFrom = null;
		map.searchAnchor = null;
		map.paletteOpen = true;
		close();
	}

	function connectFrom(id: number) {
		map.linkFrom = id;
		// Anchor on the source node: the placement helper steps out from there and owns the
		// spacing, so every way of adding a system leaves the same gap.
		const source = map.systems.find((sys) => sys.id === id);
		map.searchAnchor = source ? { x: source.position_x, y: source.position_y } : null;
		map.paletteOpen = true;
		close();
	}

	function setStatus(id: number, status: SystemStatus) {
		map.run('setStatus', api.setStatus({ map_id: map.mapId, map_solar_system_id: id, status }));
		close();
	}

	function togglePin(id: number, value: boolean) {
		map.run('setPinned', api.setPinned({ map_id: map.mapId, map_solar_system_id: id, value }));
		close();
	}

	function toggleHome(id: number, value: boolean) {
		map.run('setHome', api.setHome({ map_id: map.mapId, map_solar_system_id: id, value }));
		close();
	}

	function toggleRally(id: number, value: boolean) {
		map.run('setRally', api.setRally({ map_id: map.mapId, map_solar_system_id: id, value }));
		close();
	}

	/** Removes the whole marquee selection when one exists, else just this node. */
	function removeSystem(id: number) {
		const ids = map.selected.size > 0 ? [...map.selected] : [id];
		map.selected = new Set();
		map.run('removeSystems', api.removeSystems({ map_id: map.mapId, map_solar_system_ids: ids }));
		close();
	}

	const onlineCharacters = $derived(map.myCharacters.filter((c) => c.online));

	function waypoint(characterId: number, destinationId: number, clearOthers: boolean) {
		map.run(
			'setWaypoint',
			api.setWaypoint({
				character_id: characterId,
				destination_id: destinationId,
				clear_other_waypoints: clearOthers,
			}),
		);
		close();
	}

	function waypointAll(destinationId: number, clearOthers: boolean) {
		map.run(
			'setWaypoint',
			api.setWaypointAll({ destination_id: destinationId, clear_other_waypoints: clearOthers }),
		);
		close();
	}
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

{#if s.kind === 'ghost'}
	<button class={item} onclick={() => assignSystem(s.id)}>
		<SearchIcon class="size-4" />
		Assign a system
	</button>
	<div class="my-0.5 border-t border-border"></div>
{/if}
{#if s.kind === 'system'}
	<button class={item} onclick={() => togglePin(s.id, !s.is_pinned)}>
		<PinIcon class="size-4" />
		{s.is_pinned ? 'Unpin' : 'Pin'}
	</button>
	<button class={item} onclick={() => connectFrom(s.id)}>
		<WaypointsIcon class="size-4" />
		Add connection
	</button>
{/if}

{#if s.kind === 'system'}
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
			<a class={item} href={dotlanSystemUrl(s.name)} target="_blank" rel="noopener">
				<GlobeIcon class="size-4" /> System
			</a>
			<a class={item} href={dotlanRegionMapUrl(s.region, s.name)} target="_blank" rel="noopener">
				<MapIcon class="size-4" /> Region Map
			</a>
			{#if !isWormholeClass(s.wormhole_class_id)}
				<a class={item} href={dotlanJumpRangeUrl(s.name)} target="_blank" rel="noopener">
					<CompassIcon class="size-4" /> Jump Range
				</a>
			{/if}
			<div class="my-0.5 border-t border-border"></div>
			<div
				class="px-3 py-1 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase"
			>
				zKillboard
			</div>
			<a class={item} href={zkillboardSystemUrl(s.solar_system_id)} target="_blank" rel="noopener">
				<GlobeIcon class="size-4" /> System
			</a>
			<a
				class={item}
				href={zkillboardConstellationUrl(s.constellation_id)}
				target="_blank"
				rel="noopener"
			>
				<CompassIcon class="size-4" /> Constellation
			</a>
			<a class={item} href={zkillboardRegionUrl(s.region_id)} target="_blank" rel="noopener">
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
{/if}

{#if !s.is_pinned && !s.is_home}
	<div class="my-0.5 border-t border-border"></div>
	<button class="{item} text-destructive hover:text-destructive" onclick={() => removeSystem(s.id)}>
		<Trash2Icon class="size-4" />
		Remove
	</button>
{/if}
