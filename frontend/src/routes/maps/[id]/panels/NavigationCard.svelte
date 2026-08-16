<script lang="ts">
	// Route planner (legacy Navigation "Route" tab): origin/destination pickers, a
	// preference select, and the computed route with per-jump rows. The path also drives
	// the connection highlight on the canvas.
	import ArrowLeftRightIcon from '@lucide/svelte/icons/arrow-left-right';
	import XIcon from '@lucide/svelte/icons/x';

	import { browser } from '$app/environment';

	import { api } from '$lib/api/client';
	import { Button } from '$lib/components/ui/button';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import RouteList from '$lib/components/map/RouteList.svelte';
	import * as Select from '$lib/components/ui/select';
	import {
		buildAdjacency,
		findRoute,
		type RouteGraph,
		type RoutePreference
	} from '$lib/routing/algorithm';
	import type { MapState } from '../map-state.svelte';
	import SystemCombobox from '$lib/components/pickers/SystemCombobox.svelte';

	let { map }: { map: MapState } = $props();

	const PREFS: { value: RoutePreference; label: string }[] = [
		{ value: 'shorter', label: 'Shortest' },
		{ value: 'safer', label: 'Safer' },
		{ value: 'less_secure', label: 'Less Secure' }
	];
	let preference = $state<RoutePreference>(
		browser ? ((localStorage.getItem('route-preference') as RoutePreference) ?? 'shorter') : 'shorter'
	);
	$effect(() => {
		localStorage.setItem('route-preference', preference);
	});

	// The static graph loads once per page.
	let stargates = $state<Map<number, number[]> | null>(null);
	let security = $state<Map<number, number>>(new Map());
	$effect(() => {
		api
			.routingGraph()
			.then((g) => {
				stargates = new Map(
					Object.entries(g.adjacency).map(([k, v]) => [Number(k), v as number[]])
				);
				security = new Map(Object.entries(g.security).map(([k, v]) => [Number(k), v]));
			})
			.catch(() => {});
	});

	// Live wormhole adjacency from the map's connections (solar system ids).
	const wormholes = $derived.by(() => {
		const placementSystem = new Map<number, number>();
		for (const s of map.systems) placementSystem.set(s.id, s.solar_system_id);
		const edges: [number, number][] = [];
		for (const c of map.connections) {
			const a = placementSystem.get(c.from_system);
			const b = placementSystem.get(c.to_system);
			if (a !== undefined && b !== undefined && a !== b) edges.push([a, b]);
		}
		return buildAdjacency(edges);
	});

	const result = $derived.by(() => {
		if (!stargates || map.routeFromId === null || map.routeToId === null) return null;
		const graph: RouteGraph = { stargates, wormholes, security };
		return findRoute(graph, map.routeFromId, map.routeToId, {
			preference,
			securityPenalty: 50
		});
	});

	// Publish the path for the canvas highlight.
	$effect(() => {
		map.routePath = result?.route.map((s) => s.id) ?? [];
	});

	const jumpTone = $derived.by(() => {
		const j = result?.jumps ?? 0;
		if (j < 8) return 'text-green-500';
		if (j < 15) return 'text-amber-500';
		return 'text-red-500';
	});

	function swap() {
		[map.routeFromId, map.routeToId] = [map.routeToId, map.routeFromId];
	}
</script>

<MapPanel testid="navigation-card">
	<MapPanelHeader>
		Navigation
		{#snippet actions()}
			<Select.Root type="single" bind:value={preference}>
				<Select.Trigger size="sm" data-testid="route-preference">
					{PREFS.find((p) => p.value === preference)?.label}
				</Select.Trigger>
				<Select.Content>
					<Select.Group>
						{#each PREFS as p (p.value)}
							<Select.Item value={p.value}>{p.label}</Select.Item>
						{/each}
					</Select.Group>
				</Select.Content>
			</Select.Root>
		{/snippet}
	</MapPanelHeader>
	<MapPanelContent>
		<div class="flex flex-col gap-2 p-3 text-xs">
		<div class="flex items-center gap-1.5">
			<SystemCombobox
				placeholder="Origin"
				value={map.routeFromId}
				onpick={(id) => (map.routeFromId = id)}
			/>
			<Button variant="ghost" size="icon-xs" aria-label="Swap" onclick={swap}>
				<ArrowLeftRightIcon />
			</Button>
			<SystemCombobox
				placeholder="Destination"
				value={map.routeToId}
				onpick={(id) => (map.routeToId = id)}
			/>
		</div>

		{#if map.activeSystem}
			<button
				class="self-start text-[11px] text-muted-foreground underline-offset-2 hover:underline"
				onclick={() => (map.routeFromId = map.activeSystem?.solar_system_id ?? null)}
			>
				From active: {map.activeSystem.alias ?? map.activeSystem.name}
			</button>
		{/if}

		{#if map.routeFromId === null || map.routeToId === null}
			<p class="text-muted-foreground">Select origin and destination</p>
		{:else if !result}
			<p class="text-muted-foreground" data-testid="no-route">No route found</p>
		{:else}
			<div class="flex items-center justify-between font-medium">
				<span class={jumpTone} data-testid="route-jumps">{result.jumps} jumps</span>
			</div>
			<RouteList steps={result.route} />
			<Button
				variant="ghost"
				size="xs"
				class="self-start"
				onclick={() => {
					map.routeFromId = null;
					map.routeToId = null;
				}}
			>
				<XIcon data-icon="inline-start" />
				Clear route
			</Button>
		{/if}
		</div>
	</MapPanelContent>
</MapPanel>
