<script lang="ts">
	// Raidable skyhooks: which ones are open, which are about to be, and how far away.
	//
	// The whole list is shown rather than a slice near you, because a two-hour window is
	// long enough to fly a long way for. What makes that readable is sorting: distance
	// first by default, so the ones you could reach are the ones you see.
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';

	import { api } from '$lib/api/client';
	import type { PlanetKind } from '$lib/api/types/PlanetKind';
	import type { Skyhook } from '$lib/api/types/Skyhook';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import EveImage from '$lib/components/EveImage.svelte';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as ToggleGroup from '$lib/components/ui/toggle-group';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { findRoutes, jumpTone } from '$lib/routing/algorithm';
	import type { RouteResult } from '$lib/routing/algorithm';
	import {
		describe,
		formatDuration,
		formatWindow,
		statusDot,
		statusText,
		timing,
		type SkyhookStatus
	} from '$lib/skyhooks/timer';
	import { cn } from '$lib/utils';
	import type { MapState } from '../map-state.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import RoutePopover from './RoutePopover.svelte';

	let { map, layoutActions }: { map: MapState; layoutActions?: import('svelte').Snippet } =
		$props();

	type Column = 'jumps' | 'planet' | 'region' | 'timer';

	let skyhooks = $state<Skyhook[]>([]);
	let now = $state(new Date());
	// Skyhooks only go on the planets that yield reagents, so lava and ice is the whole
	// vocabulary. One at a time, because the reagent is what you went for: a mixed list
	// would need a per-row marker to say which is which, and this says it once.
	let kind = $state<PlanetKind>('lava');
	// Which of the three live states to show. Closed ones are never listed: the window is
	// over, so there is nothing to go and do.
	let shown = $state<string[]>(['upcoming', 'open', 'closing']);
	let column = $state<Column>('jumps');
	let ascending = $state(true);

	function load() {
		api
			.skyhooks()
			.then((rows) => (skyhooks = rows))
			.catch(() => {});
	}

	$effect(() => {
		load();
		// The server mirrors ESI every five minutes, so asking more often learns nothing.
		const poll = setInterval(load, 5 * 60_000);
		// Timers are read to the minute, so half a minute keeps them honest without churn.
		const clock = setInterval(() => (now = new Date()), 30_000);
		return () => {
			clearInterval(poll);
			clearInterval(clock);
		};
	});

	// One search from the origin covers every skyhook, rather than one per row.
	const routes = $derived.by<Map<number, RouteResult>>(() => {
		const graph = map.graph;
		const origin = map.routeOrigin;
		if (!graph || origin === null || skyhooks.length === 0) return new Map();
		const targets = [...new Set(skyhooks.map((s) => s.solar_system_id))];
		return findRoutes(graph, origin, targets, map.routingSettings, map.ignoredSystems);
	});

	interface Row {
		skyhook: Skyhook;
		status: SkyhookStatus;
		untilMs: number;
		route: RouteResult | undefined;
		jumps: number | null;
	}

	const rows = $derived.by<Row[]>(() =>
		skyhooks.map((skyhook) => {
			const t = timing(skyhook, now);
			const route = routes.get(skyhook.solar_system_id);
			return { skyhook, status: t.status, untilMs: t.untilMs, route, jumps: route?.jumps ?? null };
		})
	);

	const live = $derived(rows.filter((r) => r.status !== 'closed' && shown.includes(r.status)));
	const counts = $derived({
		lava: live.filter((r) => r.skyhook.planet_kind === 'lava').length,
		ice: live.filter((r) => r.skyhook.planet_kind === 'ice').length,
		other: live.filter((r) => r.skyhook.planet_kind === 'other').length
	});

	/**
	 * Lava and ice always; anything else only once there is one to show.
	 *
	 * A skyhook can only go on a reagent planet, so "other" should stay empty forever. It
	 * is still counted, because a permanent third button is clutter but a skyhook that
	 * silently cannot be displayed is a bug you would never see.
	 */
	const choices = $derived([
		{ key: 'lava' as const, label: 'Lava' },
		{ key: 'ice' as const, label: 'Ice' },
		...(counts.other > 0 ? [{ key: 'other' as const, label: 'Other' }] : [])
	]);

	/** Unreachable sorts last however the column is pointed: it is never the answer. */
	function byJumps(a: Row, b: Row) {
		if (a.jumps === null || b.jumps === null) return a.jumps === null ? 1 : -1;
		return a.jumps - b.jumps;
	}

	const sorted = $derived.by(() => {
		const direction = ascending ? 1 : -1;
		const compare: Record<Column, (a: Row, b: Row) => number> = {
			jumps: byJumps,
			planet: (a, b) => a.skyhook.planet_name.localeCompare(b.skyhook.planet_name),
			region: (a, b) => a.skyhook.region.localeCompare(b.skyhook.region),
			// Open before upcoming, then by how soon the moment is.
			timer: (a, b) => {
				const rank = (r: Row) => (r.status === 'upcoming' ? 1 : 0);
				return rank(a) - rank(b) || a.untilMs - b.untilMs;
			}
		};
		return live
			.filter((r) => r.skyhook.planet_kind === kind)
			.sort((a, b) => {
				const primary = compare[column](a, b) * direction;
				// Always break ties the same way, so the order never jitters as timers tick.
				return primary || a.skyhook.planet_id - b.skyhook.planet_id;
			});
	});

	function sortBy(next: Column) {
		if (column === next) {
			ascending = !ascending;
			return;
		}
		column = next;
		ascending = true;
	}

	/**
	 * The row's system in the shape the context menu wants.
	 *
	 * Built from the payload rather than resolved: a skyhook already carries everything the
	 * menu needs, and a row that cannot be right-clicked until a second request lands is a
	 * row that sometimes cannot be right-clicked at all.
	 */
	function systemOf(skyhook: Skyhook): SystemSearchResult {
		return {
			id: skyhook.solar_system_id,
			name: skyhook.system_name,
			security: skyhook.security_status,
			region: skyhook.region,
			region_id: skyhook.region_id,
			constellation_id: skyhook.constellation_id,
			// Skyhooks only exist in sovereign nullsec, so neither of these can apply.
			wormhole_class_id: null,
			effect_name: null,
			sovereignty: skyhook.sovereignty ?? null
		};
	}

	// EVE serves a faction's logo from the corporations endpoint keyed by the faction id,
	// so anything that is not an alliance uses the corporation one.
	function sovKind(sov: NonNullable<Skyhook['sovereignty']>) {
		return sov.kind === 'alliance' ? 'alliance' : 'corporation';
	}

	function hover(row: Row, on: boolean) {
		const placed = map.systems.find((s) => s.solar_system_id === row.skyhook.solar_system_id);
		map.hoveredSystemId = on ? (placed?.id ?? null) : null;
		map.hoverPath = on ? (row.route?.route.map((s) => s.id) ?? null) : null;
	}

	// Anything that opens within reach is worth a word, because the card may not be the
	// tile you are looking at. Only once per skyhook per window.
	const NEARBY_JUMPS = 15;
	let announced = new Set<number>();
	$effect(() => {
		for (const row of rows) {
			if (row.status !== 'open') continue;
			if (row.jumps === null || row.jumps > NEARBY_JUMPS) continue;
			if (announced.has(row.skyhook.planet_id)) continue;
			announced.add(row.skyhook.planet_id);
			map.statusLine = `${row.skyhook.planet_name} is raidable, ${row.jumps} jumps out`;
		}
		// Forget a skyhook once its window is over, so the next one is announced again.
		for (const row of rows) {
			if (row.status === 'closed') announced.delete(row.skyhook.planet_id);
		}
	});

	const FILTERS: { key: SkyhookStatus; label: string }[] = [
		{ key: 'upcoming', label: 'Upcoming' },
		{ key: 'open', label: 'Raidable now' },
		{ key: 'closing', label: 'Closing within 15m' }
	];
</script>

{#snippet heading(key: Column, label: string, extra = '')}
	<button
		class={cn('flex items-center gap-1 hover:text-foreground', extra)}
		onclick={() => sortBy(key)}
	>
		<span>{label}</span>
		{#if column === key}
			{#if ascending}<ArrowUpIcon class="size-3" />{:else}<ArrowDownIcon class="size-3" />{/if}
		{/if}
	</button>
{/snippet}

<Tooltip.Provider delayDuration={300}>
	<MapPanel testid="skyhooks-card">
		<MapPanelHeader>
			<span class="inline-flex items-center gap-2">
				Raidable Skyhooks
				<span class="font-mono text-amber-400">{live.length}</span>
			</span>
			{#snippet actions()}
				{@render layoutActions?.()}
				<!-- Tabs rather than a toggle group: the choice is exclusive, and a toggle's
				     "on" background is the same colour as its hover, so the selected one was
				     only distinguishable by accident. The line variant marks it with an
				     underline as well as full-strength text. -->
				<Tabs.Root
					value={kind}
					onValueChange={(v) => v && (kind = v as PlanetKind)}
					class="w-fit"
					data-testid="skyhook-kinds"
				>
					<Tabs.List variant="line" class="h-6">
						{#each choices as choice (choice.key)}
							<Tabs.Trigger
								value={choice.key}
								class="px-1 text-[10px]"
								data-testid="skyhook-kind-{choice.key}"
							>
								{choice.label}
								{#if counts[choice.key] > 0}
									<span class="font-mono text-amber-400">{counts[choice.key]}</span>
								{/if}
							</Tabs.Trigger>
						{/each}
					</Tabs.List>
				</Tabs.Root>
				<ToggleGroup.Root
					type="multiple"
					size="sm"
					variant="outline"
					value={shown}
					onValueChange={(v) => (shown = v)}
					data-testid="skyhook-filters"
				>
					{#each FILTERS as filter (filter.key)}
						<Tooltip.Root>
							<Tooltip.Trigger>
								{#snippet child({ props })}
									<ToggleGroup.Item
										{...props}
										value={filter.key}
										aria-label={filter.label}
										class="size-6"
									>
										<span class={cn('inline-block size-2 rounded-full', statusDot(filter.key))}
										></span>
									</ToggleGroup.Item>
								{/snippet}
							</Tooltip.Trigger>
							<Tooltip.Content>{filter.label}</Tooltip.Content>
						</Tooltip.Root>
					{/each}
				</ToggleGroup.Root>
			{/snippet}
		</MapPanelHeader>

		<MapPanelContent>
			<div class="mt-1 flex flex-col">
				<div
					class="flex items-center gap-2 px-3 pb-1 text-[10px] tracking-wider text-muted-foreground uppercase"
				>
					<span class="w-2 shrink-0"></span>
					{@render heading('planet', 'Planet', 'min-w-0 flex-1')}
					{@render heading('region', 'Region', 'min-w-0 flex-1')}
					<span class="w-4 shrink-0"></span>
					{@render heading('jumps', 'J', 'w-10 shrink-0 justify-end')}
					{@render heading('timer', 'Timer', 'w-16 shrink-0 justify-end')}
				</div>

				{#if sorted.length === 0}
					<p class="px-3 py-4 text-xs text-muted-foreground" data-testid="skyhooks-empty">
						{skyhooks.length === 0
							? 'No raidable skyhooks right now.'
							: 'Nothing matches the current filters.'}
					</p>
				{:else}
					{#each sorted as row (row.skyhook.planet_id)}
						<!-- Right-click reaches the system menu, same as anywhere else a system is
						     named: set destination, add to map, external links. -->
						{#snippet line()}
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								class="flex items-center gap-2 border-b border-border/30 px-3 py-1 text-xs last:border-b-0 hover:bg-muted/30"
								data-testid="skyhook-row"
								data-planet={row.skyhook.planet_name}
								data-status={row.status}
								onmouseenter={() => hover(row, true)}
								onmouseleave={() => hover(row, false)}
							>
								<span
									class={cn('size-2 shrink-0 rounded-full', statusDot(row.status))}
									aria-label={row.status}
								></span>

								<span class="flex min-w-0 flex-1 items-center gap-1.5">
									<ClassBadge
										classId={null}
										security={row.skyhook.security_status}
										class="shrink-0 text-[10px]"
									/>
									<span class="truncate" title={row.skyhook.planet_name}
										>{row.skyhook.planet_name}</span
									>
								</span>

								<span
									class="min-w-0 flex-1 truncate font-mono text-[10px] text-muted-foreground"
									title={row.skyhook.region}>{row.skyhook.region}</span
								>

								<span class="flex w-4 shrink-0 justify-center">
									{#if row.skyhook.sovereignty}
										{@const sov = row.skyhook.sovereignty}
										<Tooltip.Root>
											<Tooltip.Trigger class="flex">
												<EveImage
													kind={sovKind(sov)}
													id={sov.id}
													class="size-4 shrink-0 rounded-sm"
												/>
											</Tooltip.Trigger>
											<Tooltip.Content class="flex items-center gap-2">
												<EveImage kind={sovKind(sov)} id={sov.id} class="size-6 rounded-sm" />
												{sov.name}
												{#if 'ticker' in sov}({sov.ticker}){/if}
											</Tooltip.Content>
										</Tooltip.Root>
									{/if}
								</span>

								<span class="w-10 shrink-0 text-right">
									{#if row.route}
										<RoutePopover {map} steps={row.route.route}>
											<span
												class={cn('cursor-pointer font-medium tabular-nums', jumpTone(row.jumps ?? 0))}
												data-testid="skyhook-jumps">{row.jumps}j</span
											>
										</RoutePopover>
									{:else}
										<span class="text-[10px] whitespace-nowrap text-muted-foreground/60">--</span>
									{/if}
								</span>

								<Tooltip.Root>
									<Tooltip.Trigger
										class={cn(
											'w-16 shrink-0 cursor-help text-right font-mono text-[10px] font-semibold tabular-nums',
											statusText(row.status)
										)}
										data-testid="skyhook-timer"
									>
										{formatDuration(row.untilMs)}
									</Tooltip.Trigger>
									<Tooltip.Content class="flex flex-col gap-0.5">
										<span>{describe({ status: row.status, untilMs: row.untilMs })}</span>
										<span class="font-mono text-[10px] text-muted-foreground">
											{formatWindow(row.skyhook)}
										</span>
									</Tooltip.Content>
								</Tooltip.Root>
							</div>
						{/snippet}

						<SystemMenu system={systemOf(row.skyhook)}>{@render line()}</SystemMenu>
					{/each}
				{/if}
			</div>
		</MapPanelContent>
	</MapPanel>
</Tooltip.Provider>
