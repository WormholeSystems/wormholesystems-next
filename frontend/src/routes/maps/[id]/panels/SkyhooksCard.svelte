<script lang="ts">
	// Raidable skyhooks: which ones are open, which are about to be, and how far away.
	// The whole list is shown rather than a slice near you, because a two-hour window is long
	// enough to fly a long way for. Sorted by distance by default.
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import { solarSystemId } from '$lib/map/system';

	import { createQuery } from '@tanstack/svelte-query';

	import { q } from '$lib/api/queries';
	import type { PlanetKind } from '$lib/api/types/PlanetKind';
	import type { Skyhook } from '$lib/api/types/Skyhook';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import SovereigntyBadge from '$lib/components/map-ui/SovereigntyBadge.svelte';
	import RouteOriginBadge from './RouteOriginBadge.svelte';
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
		type SkyhookStatus,
	} from '$lib/skyhooks/timer';
	import { cn } from '$lib/utils';
	import type { MapState } from '../map-state.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import RoutePopover from './RoutePopover.svelte';

	let { map }: { map: MapState } = $props();

	type Column = 'jumps' | 'planet' | 'region' | 'timer';

	// The server mirrors ESI every five minutes, so the query's own interval asks no more
	// often than that.
	const skyhooksQuery = createQuery(() => q.skyhooks());
	const skyhooks = $derived(skyhooksQuery.data ?? []);
	let now = $state(new Date());
	// Skyhooks only go on reagent planets, so lava and ice is the whole vocabulary. Shown one
	// kind at a time, because the reagent is what the trip was for.
	let kind = $state<PlanetKind>('lava');
	// Closed skyhooks are never listed: the window is over, so there is nothing to go and do.
	let shown = $state<string[]>(['upcoming', 'open', 'closing']);
	let column = $state<Column>('jumps');
	let ascending = $state(true);

	$effect(() => {
		// Timers are read to the minute, so half a minute keeps them honest without churn.
		const clock = setInterval(() => (now = new Date()), 30_000);
		return () => clearInterval(clock);
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
		}),
	);

	const live = $derived(rows.filter((r) => r.status !== 'closed' && shown.includes(r.status)));
	const counts = $derived({
		lava: live.filter((r) => r.skyhook.planet_kind === 'lava').length,
		ice: live.filter((r) => r.skyhook.planet_kind === 'ice').length,
		other: live.filter((r) => r.skyhook.planet_kind === 'other').length,
	});

	/**
	 * "Other" should stay empty forever (skyhooks only go on reagent planets), but it is still
	 * counted so one that cannot be displayed shows up instead of vanishing.
	 */
	const choices = $derived([
		{ key: 'lava' as const, label: 'Lava' },
		{ key: 'ice' as const, label: 'Ice' },
		...(counts.other > 0 ? [{ key: 'other' as const, label: 'Other' }] : []),
	]);

	/** Unreachable sorts last however the column is pointed: it is never the answer. */
	function byJumps(a: Row, b: Row) {
		if (a.jumps === null || b.jumps === null) return a.jumps === null ? 1 : -1;
		return a.jumps - b.jumps;
	}

	const sorted = $derived.by(() => {
		const direction = ascending ? 1 : -1;
		const compare = {
			jumps: byJumps,
			planet: (a, b) => a.skyhook.planet_name.localeCompare(b.skyhook.planet_name),
			region: (a, b) => a.skyhook.region.localeCompare(b.skyhook.region),
			// Open before upcoming, then by how soon the moment is.
			timer: (a, b) => {
				const rank = (r: Row) => (r.status === 'upcoming' ? 1 : 0);
				return rank(a) - rank(b) || a.untilMs - b.untilMs;
			},
		} satisfies Record<Column, (a: Row, b: Row) => number>;
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
	 * Built from the payload rather than resolved: a skyhook carries everything the menu
	 * needs, so no row waits on a second request before it can be right-clicked.
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
			sovereignty: skyhook.sovereignty ?? null,
			// Skyhooks are nullsec; a k-space system has no statics.
			statics: [],
		};
	}

	function hover(row: Row, on: boolean) {
		const placed = map.systems.find((s) => solarSystemId(s) === row.skyhook.solar_system_id);
		map.hoveredSystemId = on ? (placed?.id ?? null) : null;
		map.hoverPath = on ? (row.route?.route.map((s) => s.id) ?? null) : null;
	}

	const FILTERS: { key: SkyhookStatus; label: string }[] = [
		{ key: 'upcoming', label: 'Upcoming' },
		{ key: 'open', label: 'Raidable now' },
		{ key: 'closing', label: 'Closing within 15m' },
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
				<RouteOriginBadge {map} />
				<!-- Tabs rather than a toggle group: a toggle's "on" background is the same colour
				     as its hover, so the selected one was only distinguishable by accident. -->
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
										<SovereigntyBadge sovereignty={row.skyhook.sovereignty} />
									{/if}
								</span>

								<span class="w-10 shrink-0 text-right">
									{#if row.route}
										<RoutePopover {map} steps={row.route.route}>
											<span
												class={cn(
													'cursor-pointer font-medium tabular-nums',
													jumpTone(row.jumps ?? 0),
												)}
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
											statusText(row.status),
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
