<script lang="ts">
	// Who is where: every pilot sharing their location on this map, and how far away.
	//
	// The list is flat rather than grouped, so it reads the same whether two people are on
	// or twenty. What keeps it useful at twenty is the ordering: pilots who can act come
	// first, and the ones who cannot (docked, in a pod, sitting in a scanning frigate) sink
	// without being hidden, because "my alt is docked in D2" is still worth knowing.
	//
	// Distance is measured from the same origin as the watchlist and the Find tools, so
	// every number on the page agrees about where "here" is.
	import { api } from '$lib/api/client';
	import type { MapCharacter } from '$lib/api/types/MapCharacter';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import EveImage from '$lib/components/EveImage.svelte';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { findRoutes, jumpTone } from '$lib/routing/algorithm';
	import type { RouteResult } from '$lib/routing/algorithm';
	import { isIdle, isScanner, orderPilots } from '$lib/characters/order';
	import { cn } from '$lib/utils';
	import type { MapState } from '../map-state.svelte';
	import RoutePopover from './RoutePopover.svelte';

	let { map, layoutActions }: { map: MapState; layoutActions?: import('svelte').Snippet } =
		$props();

	const pilots = $derived(map.characters);
	const sorted = $derived(orderPilots(pilots));

	// Systems the pilots are in, resolved for the ones that are not on the map. A pilot in
	// known space is common and should still show a name rather than an id.
	let resolved = $state<Map<number, SystemSearchResult>>(new Map());
	$effect(() => {
		const placed = new Set(map.systems.map((s) => s.solar_system_id));
		const missing = [
			...new Set(
				pilots
					.map((p) => p.solar_system_id)
					.filter((id): id is number => id !== null && !placed.has(id) && !resolved.has(id))
			)
		];
		if (missing.length === 0) return;
		api
			.resolveSystems(missing)
			.then((rows) => {
				const next = new Map(resolved);
				for (const r of rows) next.set(r.id, r);
				resolved = next;
			})
			.catch(() => {});
	});

	/** Everything a row needs to name a pilot's location, on the map or off it. */
	function place(solarSystemId: number | null) {
		if (solarSystemId === null) return null;
		const placed = map.systems.find((s) => s.solar_system_id === solarSystemId);
		if (placed) {
			return {
				id: placed.id,
				alias: placed.alias,
				name: placed.name,
				region: placed.region,
				classId: placed.wormhole_class_id,
				security: placed.security_status
			};
		}
		const hit = resolved.get(solarSystemId);
		return hit
			? {
					id: null,
					alias: null,
					name: hit.name,
					region: hit.region,
					classId: hit.wormhole_class_id,
					security: hit.security
				}
			: null;
	}

	// One search from the origin covers every pilot, rather than one per row.
	const routes = $derived.by<Map<number, RouteResult>>(() => {
		const graph = map.graph;
		const origin = map.routeOrigin;
		if (!graph || origin === null) return new Map();
		const targets = [
			...new Set(pilots.map((p) => p.solar_system_id).filter((id): id is number => id !== null))
		];
		if (targets.length === 0) return new Map();
		return findRoutes(graph, origin, targets, map.routingSettings, map.ignoredSystems);
	});

	/** Highlight the pilot's node and draw their route while the row is hovered. */
	function hover(pilot: MapCharacter, on: boolean) {
		const target = pilot.solar_system_id;
		const placed = target === null ? null : map.systems.find((s) => s.solar_system_id === target);
		map.hoveredSystemId = on ? (placed?.id ?? null) : null;
		const route = target === null ? undefined : routes.get(target);
		map.hoverPath = on ? (route?.route.map((s) => s.id) ?? null) : null;
	}
</script>

<!-- The badges and the ship name are tooltips, which need a provider in scope; a card can
     be mounted anywhere in the grid, so it brings its own rather than assuming one. -->
<Tooltip.Provider delayDuration={300}>
	<MapPanel testid="characters-card">
		<MapPanelHeader>
			<span class="inline-flex items-center gap-2">
				<span class="size-1.5 animate-pulse rounded-full bg-green-500"></span>
				Pilots
				<span class="font-mono text-amber-400">{pilots.length}</span>
			</span>
			{#snippet actions()}
				{@render layoutActions?.()}
			{/snippet}
		</MapPanelHeader>
		<MapPanelContent>
			{#if pilots.length === 0}
				<p class="px-3 py-4 text-xs text-muted-foreground" data-testid="pilots-empty">
					No pilots online. Anyone who turns on Share location for this map shows up here.
				</p>
			{:else}
				<div class="flex flex-col">
					{#each sorted as pilot (pilot.character_id)}
						{@const where = place(pilot.solar_system_id)}
						{@const route = pilot.solar_system_id === null
							? undefined
							: routes.get(pilot.solar_system_id)}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class={cn(
								'flex items-center gap-2 border-b border-border/30 px-3 py-1 text-xs last:border-b-0 hover:bg-muted/30',
								isIdle(pilot) && 'opacity-50'
							)}
							data-testid="pilot-row"
							data-pilot={pilot.name}
							onmouseenter={() => hover(pilot, true)}
							onmouseleave={() => hover(pilot, false)}
						>
							<EveImage kind="character" id={pilot.character_id} class="size-5 shrink-0 rounded" />

							<span class="flex min-w-0 shrink-0 basis-28 items-center gap-1">
								<span
									class={cn('truncate', pilot.is_mine && 'font-medium text-foreground')}
									title={pilot.name}>{pilot.name}</span
								>
								{#if pilot.is_docked}
									<Tooltip.Root>
										<Tooltip.Trigger class="shrink-0 text-[10px] text-muted-foreground">
											(D)
										</Tooltip.Trigger>
										<Tooltip.Content>Docked</Tooltip.Content>
									</Tooltip.Root>
								{:else if isScanner(pilot)}
									<Tooltip.Root>
										<Tooltip.Trigger class="shrink-0 text-[10px] text-amber-400">(S)</Tooltip.Trigger>
										<Tooltip.Content>Scanner</Tooltip.Content>
									</Tooltip.Root>
								{/if}
							</span>

							<span class="flex min-w-0 flex-1 items-center gap-1.5">
								{#if pilot.ship_type_id !== null}
									<EveImage kind="type" id={pilot.ship_type_id} class="size-4 shrink-0" />
								{/if}
								<Tooltip.Root>
									<Tooltip.Trigger class="min-w-0 truncate font-mono text-[10px] text-muted-foreground">
										{pilot.ship_type ?? 'Unknown ship'}
									</Tooltip.Trigger>
									<Tooltip.Content>{pilot.ship_name ?? 'Unnamed'}</Tooltip.Content>
								</Tooltip.Root>
							</span>

							<span class="flex min-w-0 flex-1 items-center gap-1.5">
								{#if where}
									<ClassBadge
										classId={where.classId}
										security={where.security}
										class="shrink-0 text-[10px]"
									/>
									<button
										class="min-w-0 truncate text-left hover:text-foreground"
										title="{where.name} · {where.region}"
										disabled={where.id === null}
										onclick={() => where.id !== null && (map.activeId = where.id)}
									>
										{#if where.alias}<span class="font-medium">{where.alias}</span> · {/if}{where.name}
									</button>
								{:else}
									<span class="text-muted-foreground/60">Unknown</span>
								{/if}
							</span>

							<span class="shrink-0 text-right">
								{#if route}
									<RoutePopover {map} steps={route.route}>
										<span
											class={cn('cursor-pointer font-medium tabular-nums', jumpTone(route.jumps))}
											data-testid="pilot-jumps"
										>
											{route.jumps}j
										</span>
									</RoutePopover>
								{:else}
									<!-- `nowrap`: a lone hyphen pair wraps and makes the row a line taller. -->
									<span class="text-[10px] whitespace-nowrap text-muted-foreground/60">--</span>
								{/if}
							</span>
						</div>
					{/each}
				</div>
			{/if}
		</MapPanelContent>
	</MapPanel>
</Tooltip.Provider>
