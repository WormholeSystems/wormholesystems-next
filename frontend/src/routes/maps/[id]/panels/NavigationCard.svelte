<script lang="ts">
	// The navigation panel, redesigned from the legacy three-tab layout: the A→B route
	// planner is always on top, the shared watchlist below it, and the closest-systems
	// Find section at the bottom. One origin drives watchlist and Find distances: the
	// route From, else the active system, else the tracked character's location.
	import ArrowLeftRightIcon from '@lucide/svelte/icons/arrow-left-right';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import PinIcon from '@lucide/svelte/icons/pin';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import XIcon from '@lucide/svelte/icons/x';
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';

	import { browser } from '$app/environment';

	import { api } from '$lib/api/client';
	import type { MassStatus } from '$lib/api/types/MassStatus';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import type { TimeStatus } from '$lib/api/types/TimeStatus';
	import type { WatchlistEntry } from '$lib/api/types/WatchlistEntry';
	import { Button } from '$lib/components/ui/button';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import RouteList from '$lib/components/map/RouteList.svelte';
	import SystemCombobox from '$lib/components/pickers/SystemCombobox.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import { classMeta } from '$lib/map/classes';
	import {
		buildDynamicAdjacency,
		findClosestSystems,
		findRoute,
		findRoutes,
		type DynamicEdge,
		type RouteGraph,
		type RouteResult,
		type RoutingSettings
	} from '$lib/routing/algorithm';
	import type { MapState } from '../map-state.svelte';
	import RoutePopover from './RoutePopover.svelte';
	import RouteSettings from './RouteSettings.svelte';

	let { map }: { map: MapState } = $props();

	const PREF_LABELS: Record<string, string> = {
		shorter: 'Shortest',
		safer: 'Safer',
		less_secure: 'Less Secure'
	};
	const canWrite = $derived((map.data?.role ?? 'viewer') !== 'viewer');

	const routingSettings = $derived<RoutingSettings>({
		preference: (map.userSettings?.route_preference ?? 'shorter') as RoutingSettings['preference'],
		securityPenalty: map.userSettings?.security_penalty ?? 50,
		allowTimeStatus: (map.userSettings?.route_allow_time_status ?? 'critical') as TimeStatus,
		allowMassStatus: (map.userSettings?.route_allow_mass_status ?? 'reduced') as MassStatus
	});
	const useEveScout = $derived(map.userSettings?.route_use_evescout ?? false);

	// --- graph assembly ---
	let stargates = $state<Map<number, number[]> | null>(null);
	let security = $state<Map<number, number>>(new Map());
	let joveSystems = $state<Set<number>>(new Set());
	let stationSystems = $state<Set<number>>(new Set());
	$effect(() => {
		api
			.routingGraph()
			.then((g) => {
				stargates = new Map(
					Object.entries(g.adjacency).map(([k, v]) => [Number(k), v as number[]])
				);
				security = new Map(Object.entries(g.security).map(([k, v]) => [Number(k), v]));
				joveSystems = new Set(g.jove);
				stationSystems = new Set(g.stations);
			})
			.catch(() => {});
	});

	// EVE Scout edges refresh every 5 minutes while enabled.
	$effect(() => {
		if (!useEveScout) return;
		map.loadEveScout();
		const t = setInterval(() => map.loadEveScout(), 300_000);
		return () => clearInterval(t);
	});

	const graph = $derived.by<RouteGraph | null>(() => {
		if (!stargates) return null;
		const placementSystem = new Map<number, number>();
		for (const s of map.systems) placementSystem.set(s.id, s.solar_system_id);
		const edges: DynamicEdge[] = [];
		for (const c of map.connections) {
			if (c.kind !== 'wormhole') continue;
			const a = placementSystem.get(c.from_system);
			const b = placementSystem.get(c.to_system);
			if (a === undefined || b === undefined || a === b) continue;
			edges.push({ a, b, via: 'wormhole', mass: c.mass_status, time: c.time_status });
		}
		if (useEveScout) {
			for (const e of map.eveScout) {
				edges.push({
					a: e.from_solar_system_id,
					b: e.to_solar_system_id,
					via: 'evescout',
					mass: e.mass_status as MassStatus,
					time: e.time_status as TimeStatus
				});
			}
		}
		return { stargates, dynamic: buildDynamicAdjacency(edges), security };
	});

	// --- display-data resolution cache (watchlist rows, chips, origin label, find) ---
	let resolved = $state<Map<number, SystemSearchResult>>(new Map());
	function needResolve(ids: number[]) {
		const missing = ids.filter((id) => !resolved.has(id));
		if (missing.length === 0) return;
		api
			.resolveSystems(missing)
			.then((rows) => {
				const next = new Map(resolved);
				for (const r of rows) next.set(r.id, r);
				resolved = next;
			})
			.catch(() => {});
	}

	// --- A→B route ---
	const abResult = $derived.by(() => {
		if (!graph || map.routeFromId === null || map.routeToId === null) return null;
		return findRoute(graph, map.routeFromId, map.routeToId, routingSettings, map.ignoredSystems);
	});
	const abPath = $derived(abResult?.route.map((s) => s.id) ?? []);
	// Watchlist-row hover temporarily overrides the pinned A→B highlight.
	let hoverPath = $state<number[] | null>(null);
	$effect(() => {
		map.routePath = hoverPath ?? abPath;
	});
	const jumpTone = $derived.by(() => {
		const j = abResult?.jumps ?? 0;
		if (j < 8) return 'text-green-500';
		if (j < 15) return 'text-amber-500';
		return 'text-red-500';
	});

	// --- quick picks ---
	const quickPicks = $derived.by(() => {
		const picks: { id: number; label: string; icon: 'active' | 'character' | 'pinned' }[] = [];
		const active = map.activeSystem;
		if (active) {
			picks.push({ id: active.solar_system_id, label: active.alias ?? active.name, icon: 'active' });
		}
		const character = map.myCharacters.find((c) => c.online && c.solar_system_id !== null);
		if (character?.solar_system_id != null && !picks.some((p) => p.id === character.solar_system_id)) {
			picks.push({
				id: character.solar_system_id,
				label: resolved.get(character.solar_system_id)?.name ?? String(character.solar_system_id),
				icon: 'character'
			});
		}
		for (const entry of map.watchlist.filter((w) => w.is_pinned).slice(0, 3)) {
			if (picks.some((p) => p.id === entry.solar_system_id)) continue;
			picks.push({
				id: entry.solar_system_id,
				label: resolved.get(entry.solar_system_id)?.name ?? String(entry.solar_system_id),
				icon: 'pinned'
			});
		}
		return picks;
	});
	$effect(() => {
		needResolve(quickPicks.map((p) => p.id));
	});
	function pickInto(id: number) {
		if (map.routeFromId === null) map.routeFromId = id;
		else map.routeToId = id;
	}

	// --- watchlist ---
	const origin = $derived(map.routeOrigin);
	const originName = $derived(origin === null ? null : (resolved.get(origin)?.name ?? '…'));
	$effect(() => {
		needResolve([
			...(origin === null ? [] : [origin]),
			...map.watchlist.map((w) => w.solar_system_id)
		]);
	});

	const watchRoutes = $derived.by(() => {
		if (!graph || origin === null) return new Map<number, RouteResult>();
		return findRoutes(
			graph,
			origin,
			map.watchlist.map((w) => w.solar_system_id),
			routingSettings,
			map.ignoredSystems
		);
	});

	type SortColumn = 'system' | 'region' | 'jumps';
	let sort = $state<{ column: SortColumn; direction: 'asc' | 'desc' }>(
		(browser && JSON.parse(localStorage.getItem('watchlist-sort') ?? 'null')) || {
			column: 'system',
			direction: 'asc'
		}
	);
	$effect(() => {
		localStorage.setItem('watchlist-sort', JSON.stringify(sort));
	});
	function handleSort(column: SortColumn) {
		sort =
			sort.column === column
				? { column, direction: sort.direction === 'asc' ? 'desc' : 'asc' }
				: { column, direction: 'asc' };
	}

	function jumpsOf(entry: WatchlistEntry): number | null {
		return watchRoutes.get(entry.solar_system_id)?.jumps ?? null;
	}

	const sortedWatchlist = $derived.by(() => {
		const dir = sort.direction === 'asc' ? 1 : -1;
		return map.watchlist.toSorted((a, b) => {
			const ra = resolved.get(a.solar_system_id);
			const rb = resolved.get(b.solar_system_id);
			let cmp = 0;
			switch (sort.column) {
				case 'system': {
					const wa = ra ? classMeta(ra.wormhole_class_id, ra.security).sortWeight : 99;
					const wb = rb ? classMeta(rb.wormhole_class_id, rb.security).sortWeight : 99;
					cmp = wa - wb || (ra?.name ?? '').localeCompare(rb?.name ?? '');
					break;
				}
				case 'region':
					cmp = (ra?.region ?? '').localeCompare(rb?.region ?? '');
					break;
				case 'jumps':
					cmp = (jumpsOf(a) ?? 999) - (jumpsOf(b) ?? 999);
					break;
			}
			return cmp * dir;
		});
	});

	function badgeTone(jumps: number): string {
		if (jumps < 8) return 'text-green-400';
		if (jumps < 15) return 'text-amber-400';
		return 'text-red-400';
	}

	// --- add-to-watchlist search (header plus) ---
	let addOpen = $state(false);
	let addQuery = $state('');
	let addResults = $state<SystemSearchResult[]>([]);
	let addGeneration = 0;
	$effect(() => {
		const text = addQuery;
		const request = ++addGeneration;
		api
			.searchSystems(text)
			.then((found) => {
				if (addGeneration === request) addResults = found;
			})
			.catch(() => {});
	});
	function addToWatchlist(id: number) {
		map.run('watch', api.addWatchlistEntry({ map_id: map.mapId, solar_system_id: id }));
		addOpen = false;
	}

	// --- Find (closest systems) ---
	let findOpen = $state(false);
	const CONDITIONS = [
		{ value: 'observatories', label: 'Jove Observatories' },
		{ value: 'npc_stations', label: 'NPC Stations' },
		{ value: 'highsec', label: 'High Security' },
		{ value: 'lowsec', label: 'Low Security' },
		{ value: 'nullsec', label: 'Null Security' }
	];
	let condition = $state('observatories');
	let findLimit = $state('15');
	const findResults = $derived.by(() => {
		if (!findOpen || !graph || origin === null) return [];
		const sec = (id: number) => security.get(id) ?? 0;
		const matchers: Record<string, (id: number) => boolean> = {
			observatories: (id) => joveSystems.has(id),
			npc_stations: (id) => stationSystems.has(id),
			highsec: (id) => sec(id) >= 0.5,
			lowsec: (id) => sec(id) >= 0.1 && sec(id) <= 0.4,
			nullsec: (id) => sec(id) <= 0
		};
		return findClosestSystems(
			graph,
			origin,
			matchers[condition] ?? (() => false),
			Number(findLimit),
			routingSettings,
			map.ignoredSystems
		);
	});
	$effect(() => {
		needResolve(findResults.map((r) => r.id));
	});

	function swap() {
		[map.routeFromId, map.routeToId] = [map.routeToId, map.routeFromId];
	}
</script>

<MapPanel testid="navigation-card">
	<MapPanelHeader>
		Navigation
		<span class="ml-1 text-muted-foreground/60 normal-case">
			{PREF_LABELS[map.userSettings?.route_preference ?? 'shorter']}
		</span>
		{#snippet actions()}
			<RouteSettings {map} />
			{#if canWrite}
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
							<Command.List class="max-h-48">
								<Command.Empty>
									{addQuery.trim().length < 2
										? 'Type at least two characters to search.'
										: 'No systems found.'}
								</Command.Empty>
								<Command.Group>
									{#each addResults as s (s.id)}
										<Command.Item value={String(s.id)} onSelect={() => addToWatchlist(s.id)}>
											<SystemRow system={s} />
										</Command.Item>
									{/each}
								</Command.Group>
							</Command.List>
						</Command.Root>
					</Popover.Content>
				</Popover.Root>
			{/if}
		{/snippet}
	</MapPanelHeader>
	<MapPanelContent>
		<!-- Route planner: always on top, no tab switch needed. -->
		<div class="flex flex-col gap-2 border-b border-border/50 p-3 text-xs">
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

			{#if (map.routeFromId === null || map.routeToId === null) && quickPicks.length > 0}
				<div class="flex flex-wrap gap-1.5" data-testid="quick-picks">
					{#each quickPicks as pick (pick.id)}
						<button
							class="inline-flex items-center gap-1.5 rounded-md border border-border/40 bg-muted/30 px-2 py-1 text-xs transition-colors hover:bg-muted/60"
							onclick={() => pickInto(pick.id)}
						>
							{pick.label}
							{#if pick.icon === 'active'}
								<MapPinIcon class="size-3 text-muted-foreground" />
							{:else if pick.icon === 'character'}
								<NavigationIcon class="size-3 text-muted-foreground" />
							{/if}
						</button>
					{/each}
				</div>
			{/if}

			{#if abResult === null && map.routeFromId !== null && map.routeToId !== null}
				<p class="text-muted-foreground" data-testid="no-route">No route found</p>
			{:else if abResult}
				<div class="flex items-center justify-between font-medium">
					<span class={jumpTone} data-testid="route-jumps">{abResult.jumps} jumps</span>
					<span class="flex items-center gap-2">
						{#if map.ignoredSystems.size > 0}
							<button
								class="text-[11px] text-muted-foreground underline-offset-2 hover:underline"
								data-testid="clear-ignored"
								onclick={() => map.clearIgnored()}
							>
								{map.ignoredSystems.size} ignored · Clear
							</button>
						{/if}
						<Button
							variant="ghost"
							size="icon-xs"
							aria-label="Clear route"
							onclick={() => {
								map.routeFromId = null;
								map.routeToId = null;
							}}
						>
							<XIcon />
						</Button>
					</span>
				</div>
				<RouteList steps={abResult.route} onignore={(id) => map.ignoreSystem(id)} />
			{:else if map.ignoredSystems.size > 0}
				<button
					class="self-start text-[11px] text-muted-foreground underline-offset-2 hover:underline"
					data-testid="clear-ignored"
					onclick={() => map.clearIgnored()}
				>
					{map.ignoredSystems.size} ignored · Clear
				</button>
			{/if}
		</div>

		<!-- Watchlist -->
		<div class="flex flex-col">
			<div
				class="flex items-center gap-2 border-b border-border/30 bg-muted/20 px-3 py-1.5 font-mono text-[10px] tracking-wider text-muted-foreground uppercase"
			>
				{#snippet arrow(column: SortColumn)}
					{#if sort.column === column}
						{#if sort.direction === 'asc'}
							<ArrowUpIcon class="size-3" />
						{:else}
							<ArrowDownIcon class="size-3" />
						{/if}
					{/if}
				{/snippet}
				<button
					class="flex min-w-0 flex-1 items-center gap-1 hover:text-foreground"
					onclick={() => handleSort('system')}
				>
					<span class="truncate">
						Watchlist{#if originName}&nbsp;· from {originName}{/if}
					</span>
					{@render arrow('system')}
				</button>
				<button
					class="flex w-20 shrink-0 items-center gap-1 hover:text-foreground"
					onclick={() => handleSort('region')}>Region {@render arrow('region')}</button
				>
				<button
					class="flex w-8 shrink-0 items-center justify-end gap-1 hover:text-foreground"
					onclick={() => handleSort('jumps')}>J {@render arrow('jumps')}</button
				>
				{#if canWrite}<span class="w-10 shrink-0"></span>{/if}
			</div>

			{#if sortedWatchlist.length === 0}
				<p
					class="p-3 text-center font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase"
				>
					Watchlist empty
				</p>
			{/if}
			{#each sortedWatchlist as entry (entry.id)}
				{@const r = resolved.get(entry.solar_system_id)}
				{@const route = watchRoutes.get(entry.solar_system_id)}
				<div
					class="flex items-center gap-2 border-b border-border/30 px-3 py-1 text-xs hover:bg-muted/30"
					data-testid="watchlist-row"
					role="listitem"
					onmouseenter={() => (hoverPath = route?.route.map((s) => s.id) ?? null)}
					onmouseleave={() => (hoverPath = null)}
				>
					{#if r}
						<SystemMenu system={r}>
							<SystemRow system={r} />
						</SystemMenu>
					{:else}
						<span class="min-w-0 flex-1 truncate text-muted-foreground">
							{entry.solar_system_id}
						</span>
					{/if}
					{#if route}
						<RoutePopover {map} steps={route.route}>
							<span class="cursor-pointer font-mono text-xs font-medium {badgeTone(route.jumps)}">
								{route.jumps}j
							</span>
						</RoutePopover>
					{:else}
						<span class="font-mono text-[10px] text-muted-foreground/60">--</span>
					{/if}
					{#if canWrite}
						<span class="flex w-10 shrink-0 items-center justify-end gap-1">
							<button
								class="text-muted-foreground hover:text-foreground {entry.is_pinned
									? 'text-amber-400 hover:text-amber-400'
									: ''}"
								title={entry.is_pinned ? 'Unpin' : 'Pin as quick-pick'}
								aria-label="Pin {r?.name ?? entry.solar_system_id}"
								onclick={() =>
									map.run(
										'pin',
										api.setWatchlistPinned({
											map_id: map.mapId,
											entry_id: entry.id,
											value: !entry.is_pinned
										})
									)}
							>
								<PinIcon class="size-3" />
							</button>
							<button
								class="text-muted-foreground hover:text-destructive"
								title="Remove from watchlist"
								aria-label="Remove {r?.name ?? entry.solar_system_id}"
								onclick={() =>
									map.run(
										'unwatch',
										api.removeWatchlistEntry({ map_id: map.mapId, entry_id: entry.id })
									)}
							>
								<Trash2Icon class="size-3" />
							</button>
						</span>
					{/if}
				</div>
			{/each}
		</div>

		<!-- Find: closest systems matching a condition. -->
		<div class="flex flex-col">
			<button
				class="flex items-center gap-1 border-b border-border/30 bg-muted/20 px-3 py-1.5 font-mono text-[10px] tracking-wider text-muted-foreground uppercase hover:text-foreground"
				data-testid="find-toggle"
				onclick={() => (findOpen = !findOpen)}
			>
				{#if findOpen}
					<ChevronDownIcon class="size-3" />
				{:else}
					<ChevronRightIcon class="size-3" />
				{/if}
				Find
			</button>
			{#if findOpen}
				<div class="flex items-center gap-1.5 border-b border-border/30 p-2">
					<select
						bind:value={condition}
						class="h-7 flex-1 rounded-md border border-input bg-muted/30 px-2 text-xs"
						data-testid="find-condition"
					>
						{#each CONDITIONS as c (c.value)}
							<option value={c.value}>{c.label}</option>
						{/each}
					</select>
					<select
						bind:value={findLimit}
						class="h-7 w-14 rounded-md border border-input bg-muted/30 px-1 text-xs"
						data-testid="find-limit"
					>
						{#each ['5', '10', '15', '25', '50'] as n (n)}
							<option value={n}>{n}</option>
						{/each}
					</select>
				</div>
				{#if origin === null}
					<p
						class="p-3 text-center font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase"
					>
						Select an origin
					</p>
				{:else if findResults.length === 0}
					<p
						class="p-3 text-center font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase"
					>
						No systems found
					</p>
				{/if}
				{#each findResults as result (result.id)}
					{@const r = resolved.get(result.id)}
					<div
						class="flex items-center gap-2 border-b border-border/30 px-3 py-1 text-xs hover:bg-muted/30"
						data-testid="find-row"
						role="listitem"
						onmouseenter={() => (hoverPath = result.route.map((s) => s.id))}
						onmouseleave={() => (hoverPath = null)}
					>
						{#if r}
							<SystemMenu system={r}>
								<SystemRow system={r} />
							</SystemMenu>
						{:else}
							<span class="min-w-0 flex-1 truncate text-muted-foreground">{result.id}</span>
						{/if}
						<RoutePopover {map} steps={result.route}>
							<span class="cursor-pointer font-mono text-xs font-medium {badgeTone(result.jumps)}">
								{result.jumps}j
							</span>
						</RoutePopover>
					</div>
				{/each}
			{/if}
		</div>
	</MapPanelContent>
</MapPanel>
