<script lang="ts">
	// Closest-systems Find: pick a condition, get the nearest matches from the shared origin,
	// with station groups expanding to the concrete stations.
	import BuildingIcon from '@lucide/svelte/icons/building-2';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';

	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import DestinationMenu from '$lib/components/system-menu/DestinationMenu.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import SystemRow from '../pickers/SystemRow.svelte';
	import { SYSTEM_CELLS_4, SYSTEM_LIST_ACTIONS, SYSTEM_ROW } from '../pickers/columns';
	import {
		findClosestSystems,
		jumpTone as badgeTone,
		type RouteGraph,
	} from '$lib/routing/algorithm';
	import { findMatcher } from '$lib/routing/find-conditions';
	import type { MapState } from '../state/map-state.svelte';
	import RoutePopover from './RoutePopover.svelte';

	let { map, graph, origin }: { map: MapState; graph: RouteGraph | null; origin: number | null } =
		$props();

	const serviceOptions = $derived(map.route.serviceOptions);
	const corporationOptions = $derived(map.route.corporationOptions);

	let findOpen = $state(false);
	const CONDITIONS = [
		{ value: 'observatories', label: 'Jove Observatories' },
		{ value: 'npc_stations', label: 'NPC Stations' },
		{ value: 'highsec', label: 'High Security' },
		{ value: 'lowsec', label: 'Low Security' },
		{ value: 'nullsec', label: 'Null Security' },
	];
	let condition = $state('observatories');
	let findLimit = $state('15');
	const conditionLabel = $derived(
		CONDITIONS.find((c) => c.value === condition)?.label ??
			serviceOptions.find((svc) => `service_${svc.id}` === condition)?.name ??
			corporationOptions.find((corp) => `corp_${corp.id}` === condition)?.name ??
			'Pick one',
	);
	// Whichever group of stations is being searched for, if it is one: services and owners
	// are the same question, so the results expand the same way.
	const activeService = $derived(
		serviceOptions.find((svc) => condition === `service_${svc.id}`) ??
			corporationOptions.find((corp) => condition === `corp_${corp.id}`) ??
			null,
	);

	// 185 owners is a list nobody scrolls, so it is typed into instead.
	let corpSearch = $state('');
	const matchingCorps = $derived.by(() => {
		const query = corpSearch.trim().toLowerCase();
		const all = query
			? corporationOptions.filter((corp) => corp.name.toLowerCase().includes(query))
			: corporationOptions;
		return all.slice(0, 50);
	});
	// Station lists collapse by default: a service can match a dozen stations per system,
	// which would bury the jump-ordered results.
	let expandedFind = $state<Set<number>>(new Set());
	$effect(() => {
		void condition;
		void origin;
		expandedFind = new Set();
	});
	function toggleFindRow(id: number) {
		const next = new Set(expandedFind);
		if (!next.delete(id)) next.add(id);
		expandedFind = next;
	}

	const findResults = $derived.by(() => {
		if (!findOpen || !graph || origin === null) return [];
		const matches = findMatcher(condition, {
			jove: map.route.joveSystems,
			stations: map.route.stationSystems,
			security: map.route.security,
			services: serviceOptions,
			corporations: corporationOptions,
		});
		return findClosestSystems(
			graph,
			origin,
			matches,
			Number(findLimit),
			map.routingSettings,
			map.route.ignoredSystems,
		);
	});
	$effect(() => {
		map.ensureResolved(findResults.map((r) => r.id));
	});
</script>

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
		<div class={SYSTEM_LIST_ACTIONS}>
			<div class="col-span-full flex items-center gap-1.5 border-b border-border/30 p-2">
				<Select.Root type="single" bind:value={condition}>
					<Select.Trigger class="h-7 flex-1 text-xs" data-testid="find-condition">
						{conditionLabel}
					</Select.Trigger>
					<Select.Content>
						<Select.Group>
							<Select.GroupHeading>Features</Select.GroupHeading>
							{#each CONDITIONS.slice(0, 2) as c (c.value)}
								<Select.Item value={c.value} label={c.label}>{c.label}</Select.Item>
							{/each}
						</Select.Group>
						<Select.Group>
							<Select.GroupHeading>Security</Select.GroupHeading>
							{#each CONDITIONS.slice(2) as c (c.value)}
								<Select.Item value={c.value} label={c.label}>{c.label}</Select.Item>
							{/each}
						</Select.Group>
						{#if serviceOptions.length > 0}
							<Select.Group>
								<Select.GroupHeading>Station services</Select.GroupHeading>
								{#each serviceOptions as svc (svc.id)}
									<Select.Item value="service_{svc.id}" label={svc.name}>
										{svc.name}
									</Select.Item>
								{/each}
							</Select.Group>
						{/if}
						{#if corporationOptions.length > 0}
							<Select.Group>
								<Select.GroupHeading>Station owner</Select.GroupHeading>
								<div class="px-2 pb-1">
									<Input
										bind:value={corpSearch}
										placeholder="Search owners…"
										class="h-7 text-xs"
										data-testid="find-owner-search"
										onkeydown={(ev) => ev.stopPropagation()}
									/>
								</div>
								{#each matchingCorps as corp (corp.id)}
									<Select.Item value="corp_{corp.id}" label={corp.name}>
										{corp.name}
									</Select.Item>
								{/each}
								{#if matchingCorps.length === 0}
									<p class="px-2 py-1 text-xs text-muted-foreground">No owner by that name.</p>
								{/if}
							</Select.Group>
						{/if}
					</Select.Content>
				</Select.Root>
				<Select.Root type="single" bind:value={findLimit}>
					<Select.Trigger class="h-7 w-16 text-xs" data-testid="find-limit">
						{findLimit}
					</Select.Trigger>
					<Select.Content>
						<Select.Group>
							{#each ['5', '10', '15', '25', '50'] as n (n)}
								<Select.Item value={n} label={n}>{n}</Select.Item>
							{/each}
						</Select.Group>
					</Select.Content>
				</Select.Root>
			</div>
			{#if origin === null}
				<p
					class="col-span-full p-3 text-center font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase"
				>
					Select an origin
				</p>
			{:else if findResults.length === 0}
				<p
					class="col-span-full p-3 text-center font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase"
				>
					No systems found
				</p>
			{/if}
			{#each findResults as result (result.id)}
				{@const r = map.systemInfo(result.id)}
				{@const stations = activeService?.stationsBySystem.get(result.id) ?? []}
				{@const expandable = stations.length > 0}
				<!-- The role/tabindex pair is conditional (button only when there is something
			     to expand), which the static a11y check cannot follow. -->
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<div
					class="{SYSTEM_ROW} border-b border-border/30 px-3 py-1 text-xs hover:bg-muted/30 {expandable
						? 'cursor-pointer'
						: ''}"
					data-testid="find-row"
					role={expandable ? 'button' : 'listitem'}
					tabindex={expandable ? 0 : undefined}
					aria-expanded={expandable ? expandedFind.has(result.id) : undefined}
					onclick={() => expandable && toggleFindRow(result.id)}
					onkeydown={(ev) => {
						if (expandable && (ev.key === 'Enter' || ev.key === ' ')) {
							ev.preventDefault();
							toggleFindRow(result.id);
						}
					}}
					onmouseenter={() => (map.route.hoverPath = result.route.map((s) => s.id))}
					onmouseleave={() => (map.route.hoverPath = null)}
				>
					{#if r}
						<SystemMenu system={r} class={SYSTEM_CELLS_4}>
							<SystemRow system={r} />
						</SystemMenu>
					{:else}
						<span class="col-span-4 truncate text-muted-foreground">{result.id}</span>
					{/if}
					<RoutePopover {map} steps={result.route}>
						<span class="cursor-pointer text-xs font-medium {badgeTone(result.jumps)}">
							{result.jumps}j
						</span>
					</RoutePopover>
					{#if expandable}
						<span
							class="flex items-center gap-0.5 text-[10px] text-muted-foreground"
							data-testid="find-stations-indicator"
						>
							{#if expandedFind.has(result.id)}
								<ChevronDownIcon class="size-3" />
							{:else}
								<ChevronRightIcon class="size-3" />
							{/if}
							{stations.length}
						</span>
					{:else}
						<span></span>
					{/if}
				</div>
				{#if expandedFind.has(result.id)}
					{#each stations as station (station.id)}
						<DestinationMenu destinationId={station.id} class="col-span-full">
							<!-- Hovering a station keeps its system's route highlighted. -->
							<div
								class="col-span-full flex items-center gap-2 border-b border-border/20 py-0.5 pr-3 pl-5 text-[11px] text-muted-foreground hover:bg-muted/20"
								data-testid="find-station"
								role="listitem"
								onmouseenter={() => (map.route.hoverPath = result.route.map((step) => step.id))}
								onmouseleave={() => (map.route.hoverPath = null)}
							>
								<BuildingIcon class="size-3 shrink-0 text-muted-foreground/60" />
								<span class="truncate" title={station.name}>{station.name}</span>
							</div>
						</DestinationMenu>
					{/each}
				{/if}
			{/each}
		</div>
	{/if}
</div>
