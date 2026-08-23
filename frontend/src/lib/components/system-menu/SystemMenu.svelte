<script lang="ts">
	// The app-wide solar-system context menu: wrap any rendered system reference and
	// right-click (or long-press) it. Map-dependent items appear only when a MapState is
	// provided via the 'map-state' context, so the wrapper works on non-map surfaces too.
	import CircleIcon from '@lucide/svelte/icons/circle';
	import CompassIcon from '@lucide/svelte/icons/compass';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
	import FlagIcon from '@lucide/svelte/icons/flag';
	import GlobeIcon from '@lucide/svelte/icons/globe';
	import MapIcon from '@lucide/svelte/icons/map';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import RouteIcon from '@lucide/svelte/icons/route';
	import UsersIcon from '@lucide/svelte/icons/users';
	import { getContext, type Snippet } from 'svelte';
	import { solarSystemId } from '$lib/map/system';

	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import * as ContextMenu from '$lib/components/ui/context-menu';
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
	import { centerWorld, freePosition } from '$lib/map/helpers';
	import type { MapState } from '../../../routes/maps/[id]/map-state.svelte';

	let {
		system,
		class: cls = 'contents',
		children,
	}: {
		system: SystemSearchResult;
		/** Trigger element class; `contents` keeps the wrapped fragments in their row layout. */
		class?: string;
		children: Snippet;
	} = $props();

	// A getter, so the context value registered at init survives the page swapping maps.
	const getMap = getContext<(() => MapState) | undefined>('map-state');
	const map = $derived(getMap?.());

	const isWormhole = $derived(isWormholeClass(system.wormhole_class_id));

	const canWrite = $derived(map !== undefined && map.canWrite);
	const placement = $derived(map?.systems.find((s) => solarSystemId(s) === system.id) ?? null);
	const onlineCharacters = $derived(map?.myCharacters.filter((c) => c.online) ?? []);
	const watched = $derived(map?.watchlist.some((w) => w.solar_system_id === system.id) ?? false);

	const LABEL = 'text-[0.65rem] font-semibold tracking-wider text-muted-foreground uppercase';

	function addToMap() {
		if (!map) return;
		const base = centerWorld(map.pan, map.zoom, map.viewportRect());
		const spot = freePosition(map.systems, base, map.grid);
		map.run(
			'addSystem',
			api.addSystem({
				map_id: map.mapId,
				solar_system_id: system.id,
				x: spot.x,
				y: spot.y,
				alias: null,
			}),
		);
	}

	function addToWatchlist() {
		map?.run('watch', api.addWatchlistEntry({ map_id: map.mapId, solar_system_id: system.id }));
	}

	function waypoint(characterId: number, clearOthers: boolean) {
		map?.run(
			'setWaypoint',
			api.setWaypoint({
				character_id: characterId,
				destination_id: system.id,
				clear_other_waypoints: clearOthers,
			}),
		);
	}

	function waypointAll(clearOthers: boolean) {
		map?.run(
			'setWaypoint',
			api.setWaypointAll({ destination_id: system.id, clear_other_waypoints: clearOthers }),
		);
	}

	function toggleRally() {
		if (!map || !placement) return;
		map.run(
			'setRally',
			api.setRally({
				map_id: map.mapId,
				map_solar_system_id: placement.id,
				value: !placement.is_rally,
			}),
		);
	}
</script>

{#snippet waypointItems(clearOthers: boolean)}
	{#if onlineCharacters.length === 0}
		<ContextMenu.Item disabled>No characters online</ContextMenu.Item>
	{:else}
		{#each onlineCharacters as c (c.character_id)}
			<ContextMenu.Item onclick={() => waypoint(c.character_id, clearOthers)}>
				<EveImage kind="character" id={c.character_id} size={32} class="size-5 rounded-lg" />
				{c.name}
			</ContextMenu.Item>
		{/each}
		{#if onlineCharacters.length > 1}
			<ContextMenu.Separator />
			<ContextMenu.Item onclick={() => waypointAll(clearOthers)}>
				<UsersIcon class="size-4" />
				All Characters
			</ContextMenu.Item>
		{/if}
	{/if}
{/snippet}

<ContextMenu.Root>
	<ContextMenu.Trigger class={cls} data-testid="system-menu-trigger" data-system-id={system.id}>
		{@render children()}
	</ContextMenu.Trigger>
	<ContextMenu.Content class="w-52" data-testid="system-menu">
		{#if map !== undefined && canWrite && (placement === null || !watched)}
			{#if placement === null}
				<ContextMenu.Item onclick={addToMap} data-testid="menu-add-to-map">
					<PlusIcon class="size-4" />
					Add to map
				</ContextMenu.Item>
			{/if}
			{#if !watched}
				<ContextMenu.Item onclick={addToWatchlist} data-testid="menu-add-to-watchlist">
					<EyeIcon class="size-4" />
					Add to watchlist
				</ContextMenu.Item>
			{/if}
			<ContextMenu.Separator />
		{/if}

		<ContextMenu.Sub>
			<ContextMenu.SubTrigger data-testid="menu-external">
				<ExternalLinkIcon class="size-4" />
				External
			</ContextMenu.SubTrigger>
			<ContextMenu.SubContent class="w-48">
				<ContextMenu.Label class="flex items-center gap-2 {LABEL}">
					<img src="https://evemaps.dotlan.net/favicon.ico" alt="" class="size-3.5 rounded-sm" />
					Dotlan
				</ContextMenu.Label>
				<ContextMenu.Item>
					{#snippet child({ props })}
						<a {...props} target="_blank" rel="noopener" href={dotlanSystemUrl(system.name)}>
							<GlobeIcon class="size-4" />
							System
						</a>
					{/snippet}
				</ContextMenu.Item>
				<ContextMenu.Item>
					{#snippet child({ props })}
						<a
							{...props}
							target="_blank"
							rel="noopener"
							href={dotlanRegionMapUrl(system.region, system.name)}
						>
							<MapIcon class="size-4" />
							Region Map
						</a>
					{/snippet}
				</ContextMenu.Item>
				{#if !isWormhole}
					<ContextMenu.Item>
						{#snippet child({ props })}
							<a {...props} target="_blank" rel="noopener" href={dotlanJumpRangeUrl(system.name)}>
								<CircleIcon class="size-4" />
								Jump Range
							</a>
						{/snippet}
					</ContextMenu.Item>
				{/if}
				<ContextMenu.Separator />
				<ContextMenu.Label class="flex items-center gap-2 {LABEL}">
					<img src="https://zkillboard.com/favicon.ico" alt="" class="size-3.5 rounded-sm" />
					zKillboard
				</ContextMenu.Label>
				<ContextMenu.Item>
					{#snippet child({ props })}
						<a {...props} target="_blank" rel="noopener" href={zkillboardSystemUrl(system.id)}>
							<GlobeIcon class="size-4" />
							System
						</a>
					{/snippet}
				</ContextMenu.Item>
				<ContextMenu.Item>
					{#snippet child({ props })}
						<a
							{...props}
							target="_blank"
							rel="noopener"
							href={zkillboardConstellationUrl(system.constellation_id)}
						>
							<CompassIcon class="size-4" />
							Constellation
						</a>
					{/snippet}
				</ContextMenu.Item>
				<ContextMenu.Item>
					{#snippet child({ props })}
						<a
							{...props}
							target="_blank"
							rel="noopener"
							href={zkillboardRegionUrl(system.region_id)}
						>
							<MapIcon class="size-4" />
							Region
						</a>
					{/snippet}
				</ContextMenu.Item>
			</ContextMenu.SubContent>
		</ContextMenu.Sub>

		{#if map !== undefined}
			<ContextMenu.Sub>
				<ContextMenu.SubTrigger data-testid="menu-set-destination">
					<NavigationIcon class="size-4" />
					Set destination
				</ContextMenu.SubTrigger>
				<ContextMenu.SubContent class="w-48">
					{@render waypointItems(true)}
				</ContextMenu.SubContent>
			</ContextMenu.Sub>
			<ContextMenu.Sub>
				<ContextMenu.SubTrigger data-testid="menu-add-waypoint">
					<MapPinIcon class="size-4" />
					Add waypoint
				</ContextMenu.SubTrigger>
				<ContextMenu.SubContent class="w-48">
					{@render waypointItems(false)}
				</ContextMenu.SubContent>
			</ContextMenu.Sub>
			<ContextMenu.Sub>
				<ContextMenu.SubTrigger data-testid="menu-route">
					<RouteIcon class="size-4" />
					Route planner
				</ContextMenu.SubTrigger>
				<ContextMenu.SubContent class="w-48">
					<ContextMenu.Item onclick={() => (map.routeFromId = system.id)}>
						<CompassIcon class="size-4" />
						Set as origin
					</ContextMenu.Item>
					<ContextMenu.Item onclick={() => (map.routeToId = system.id)}>
						<NavigationIcon class="size-4" />
						Set as destination
					</ContextMenu.Item>
				</ContextMenu.SubContent>
			</ContextMenu.Sub>

			{#if canWrite && placement !== null}
				<ContextMenu.Separator />
				<ContextMenu.Item onclick={toggleRally} data-testid="menu-rally">
					<FlagIcon class="size-4" />
					{placement.is_rally ? 'Clear Rally Point' : 'Set as Rally Point'}
				</ContextMenu.Item>
			{/if}
		{/if}
	</ContextMenu.Content>
</ContextMenu.Root>
