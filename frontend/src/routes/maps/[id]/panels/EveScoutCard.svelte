<script lang="ts">
	// The public wormholes out of Thera and Turnur, as EVE Scout's scouts have them.
	//
	// One hub at a time. The two lists have nothing to do with each other — a Thera hole is
	// not an alternative route to a Turnur one — and interleaving them would mean a column
	// saying which hub every row belongs to, repeated on every line, to say something a tab
	// says once.
	//
	// The jump count is the reason to look: it is measured through your own chain, so a
	// public hole three jumps from your staging is worth knowing about and one forty jumps
	// out is not. Rows sort by it by default.
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';

	import { api } from '$lib/api/client';
	import type { EveScoutConnection } from '$lib/api/types/EveScoutConnection';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import { classMeta } from '$lib/map/classes';
	import EveImage from '$lib/components/EveImage.svelte';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { timeAgo } from '$lib/format';
	import { findRoutes, jumpTone } from '$lib/routing/algorithm';
	import type { RouteResult } from '$lib/routing/algorithm';
	import { cn } from '$lib/utils';
	import type { MapState } from '../map-state.svelte';
	import RoutePopover from './RoutePopover.svelte';

	let { map }: { map: MapState } =
		$props();

	type Column = 'jumps' | 'system' | 'region' | 'signature' | 'type' | 'ttl';
	type Hub = 'Thera' | 'Turnur';

	const HUBS: Hub[] = ['Thera', 'Turnur'];

	let connections = $state<EveScoutConnection[]>([]);
	let hub = $state<Hub>('Thera');
	let column = $state<Column>('jumps');
	let ascending = $state(true);
	let now = $state(new Date());

	function load() {
		api
			.eveScout()
			.then((rows) => (connections = rows))
			.catch(() => {});
	}

	$effect(() => {
		load();
		// EVE Scout is scouted by hand, so it changes on the order of minutes at best. The
		// server caches for a minute on top of this.
		const poll = setInterval(load, 5 * 60_000);
		const clock = setInterval(() => (now = new Date()), 60_000);
		return () => {
			clearInterval(poll);
			clearInterval(clock);
		};
	});

	// The far side of every connection, resolved once for names, regions and sovereignty.
	// The hubs themselves are resolved too, so a row whose destination is the other hub
	// still renders.
	let systems = $state<Map<number, SystemSearchResult>>(new Map());
	const wanted = $derived(
		[...new Set(connections.map((c) => c.solar_system_id))].sort((a, b) => a - b).join(',')
	);
	$effect(() => {
		const ids = wanted ? wanted.split(',').map(Number) : [];
		if (ids.length === 0) return;
		api
			.resolveSystems(ids)
			.then((rows) => (systems = new Map(rows.map((r) => [r.id, r]))))
			.catch(() => {});
	});

	// One search from the origin covers every row, rather than one per destination.
	//
	// The hub itself is excluded as a stepping stone: a route to a Thera hole that goes
	// *through* Thera is measuring the wrong thing, since reaching Thera is the whole
	// problem the card is helping with.
	const routes = $derived.by<Map<number, RouteResult>>(() => {
		const graph = map.graph;
		const origin = map.routeOrigin;
		const rows = connections.filter((c) => c.hub === hub);
		if (!graph || origin === null || rows.length === 0) return new Map();
		const ignored = new Set(map.ignoredSystems);
		ignored.add(rows[0].hub_solar_system_id);
		const targets = [...new Set(rows.map((c) => c.solar_system_id))];
		return findRoutes(graph, origin, targets, map.routingSettings, ignored);
	});

	interface Row {
		connection: EveScoutConnection;
		system: SystemSearchResult | undefined;
		route: RouteResult | undefined;
		jumps: number | null;
	}

	const counts = $derived({
		Thera: connections.filter((c) => c.hub === 'Thera').length,
		Turnur: connections.filter((c) => c.hub === 'Turnur').length
	});

	/** The most recent scout report in the active hub's list. */
	const updated = $derived.by(() => {
		const stamps = connections
			.filter((c) => c.hub === hub)
			.map((c) => c.updated_at)
			.filter((s): s is string => !!s);
		if (stamps.length === 0) return null;
		return timeAgo(stamps.reduce((a, b) => (a > b ? a : b)), now);
	});

	/** Unreachable sorts last however the column is pointed: it is never the answer. */
	function byJumps(a: Row, b: Row) {
		if (a.jumps === null && b.jumps === null) return 0;
		if (a.jumps === null || b.jumps === null) return a.jumps === null ? 1 : -1;
		return a.jumps - b.jumps;
	}

	/**
	 * Legacy's ordering, which is not alphabetical: known space first and ordered by
	 * security descending, then wormholes by class. It reads the way a scout thinks — the
	 * highsec exits are what most people are looking for, and a C5 is a different kind of
	 * answer to a C1 — where an A-to-Z list would interleave all of them.
	 */
	function bySystem(a: Row, b: Row) {
		const am = classMeta(a.system?.wormhole_class_id ?? null, a.system?.security ?? 0);
		const bm = classMeta(b.system?.wormhole_class_id ?? null, b.system?.security ?? 0);
		if (am.isWormholeSpace !== bm.isWormholeSpace) return am.isWormholeSpace ? 1 : -1;
		if (am.isWormholeSpace) return am.sortWeight - bm.sortWeight;
		return (b.system?.security ?? 0) - (a.system?.security ?? 0);
	}

	const sorted = $derived.by<Row[]>(() => {
		const rows: Row[] = connections
			.filter((c) => c.hub === hub)
			.map((connection) => {
				const route = routes.get(connection.solar_system_id);
				return {
					connection,
					system: systems.get(connection.solar_system_id),
					route,
					jumps: route?.jumps ?? null
				};
			});
		const direction = ascending ? 1 : -1;
		const name = (r: Row) => r.system?.name ?? '';
		const compare: Record<Column, (a: Row, b: Row) => number> = {
			jumps: byJumps,
			system: bySystem,
			region: (a, b) => (a.system?.region ?? '').localeCompare(b.system?.region ?? ''),
			signature: (a, b) => a.connection.hub_signature.localeCompare(b.connection.hub_signature),
			type: (a, b) => (a.connection.wormhole_type ?? '').localeCompare(b.connection.wormhole_type ?? ''),
			// Soonest to collapse first: that is the one you might miss.
			ttl: (a, b) => (a.connection.remaining_hours ?? 999) - (b.connection.remaining_hours ?? 999)
		};
		return rows.sort((a, b) => {
			const primary = compare[column](a, b) * direction;
			if (primary) return primary;
			// Ties fall back to the class ordering and then the name, so a column with a lot
			// of equal values (no origin, so no jumps) still reads as a sorted list rather
			// than in whatever order EVE Scout happened to return.
			return bySystem(a, b) || name(a).localeCompare(name(b));
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

	function hover(row: Row, on: boolean) {
		const placed = map.systems.find((s) => s.solar_system_id === row.connection.solar_system_id);
		map.hoveredSystemId = on ? (placed?.id ?? null) : null;
		map.hoverPath = on ? (row.route?.route.map((s) => s.id) ?? null) : null;
	}

	/** Hours to something readable at a glance: under a day, minutes matter near the end. */
	function ttl(hours: number | undefined) {
		if (hours === undefined) return '--';
		if (hours < 1) return `${Math.max(1, Math.round(hours * 60))}m`;
		return `${Math.round(hours)}h`;
	}

	function ttlTone(hours: number | undefined) {
		if (hours === undefined) return 'text-muted-foreground/60';
		if (hours < 1) return 'text-red-500';
		if (hours < 4) return 'text-amber-500';
		return 'text-muted-foreground';
	}

	// EVE serves a faction's logo from the corporations endpoint keyed by the faction id,
	// so anything that is not an alliance uses the corporation one.
	function sovKind(sov: NonNullable<SystemSearchResult['sovereignty']>) {
		return sov.kind === 'alliance' ? 'alliance' : 'corporation';
	}
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
	<MapPanel testid="evescout-card">
		<MapPanelHeader>
			<span class="inline-flex items-center gap-2">
				EVE Scout
				{#if updated}
					<span class="font-mono text-[10px] text-muted-foreground/60" data-testid="evescout-updated">
						{updated}
					</span>
				{/if}
			</span>
			{#snippet actions()}
				<Tabs.Root
					value={hub}
					onValueChange={(v) => v && (hub = v as Hub)}
					class="w-fit"
					data-testid="evescout-hubs"
				>
					<Tabs.List variant="line" class="h-6">
						{#each HUBS as name (name)}
							<Tabs.Trigger
								value={name}
								class="px-1 text-[10px]"
								data-testid="evescout-hub-{name.toLowerCase()}"
							>
								{name}
								{#if counts[name] > 0}
									<span class="font-mono text-amber-400">{counts[name]}</span>
								{/if}
							</Tabs.Trigger>
						{/each}
					</Tabs.List>
				</Tabs.Root>
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="ghost"
								size="icon"
								class="size-6"
								href="https://www.eve-scout.com/"
								target="_blank"
								rel="noopener noreferrer"
								aria-label="Open EVE Scout"
							>
								<ExternalLinkIcon />
							</Button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>Open eve-scout.com</Tooltip.Content>
				</Tooltip.Root>
			{/snippet}
		</MapPanelHeader>

		<MapPanelContent>
			<div class="mt-1 flex flex-col">
				<div
					class="flex items-center gap-2 px-3 pb-1 text-[10px] tracking-wider text-muted-foreground uppercase"
				>
					{@render heading('system', 'System', 'min-w-0 flex-1')}
					{@render heading('region', 'Region', 'min-w-0 flex-1')}
					<span class="w-4 shrink-0"></span>
					{@render heading('signature', 'Sig', 'w-14 shrink-0')}
					{@render heading('type', 'WH', 'w-10 shrink-0')}
					{@render heading('jumps', 'J', 'w-10 shrink-0 justify-end')}
					{@render heading('ttl', 'TTL', 'w-10 shrink-0 justify-end')}
				</div>

				{#if sorted.length === 0}
					<p class="px-3 py-4 text-xs text-muted-foreground" data-testid="evescout-empty">
						{connections.length === 0
							? 'No public connections reported right now.'
							: `Nothing out of ${hub} right now.`}
					</p>
				{:else}
					{#each sorted as row (row.connection.hub_signature + row.connection.signature)}
						{@const c = row.connection}
						{#snippet line()}
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								class="flex items-center gap-2 border-b border-border/30 px-3 py-1 text-xs last:border-b-0 hover:bg-muted/30"
								data-testid="evescout-row"
								data-system={row.system?.name ?? c.solar_system_id}
								onmouseenter={() => hover(row, true)}
								onmouseleave={() => hover(row, false)}
							>
								<span class="flex min-w-0 flex-1 items-center gap-1.5">
									<ClassBadge
										classId={row.system?.wormhole_class_id ?? null}
										security={row.system?.security ?? 0}
										class="shrink-0 text-[10px]"
									/>
									<span class="truncate" title={row.system?.name}>
										{row.system?.name ?? '…'}
									</span>
								</span>

								<span
									class="min-w-0 flex-1 truncate font-mono text-[10px] text-muted-foreground"
									title={row.system?.region}
								>
									{row.system?.region ?? ''}
								</span>

								<span class="flex w-4 shrink-0 justify-center">
									{#if row.system?.sovereignty}
										{@const sov = row.system.sovereignty}
										<Tooltip.Root>
											<Tooltip.Trigger class="flex">
												<EveImage kind={sovKind(sov)} id={sov.id} class="size-4 shrink-0 rounded-sm" />
											</Tooltip.Trigger>
											<Tooltip.Content class="flex items-center gap-2">
												<EveImage kind={sovKind(sov)} id={sov.id} class="size-6 rounded-sm" />
												{sov.name}
												{#if 'ticker' in sov}({sov.ticker}){/if}
											</Tooltip.Content>
										</Tooltip.Root>
									{:else if row.system?.effect_name}
										<span
											class="font-mono text-[9px] text-muted-foreground/70"
											title={row.system.effect_name}
										>
											{row.system.effect_name.slice(0, 1)}
										</span>
									{/if}
								</span>

								<!-- The signature in the hub: what you warp to once you are there. The far
								     side's own id is on the tooltip, for the scan you will do on arrival. -->
								<Tooltip.Root>
									<Tooltip.Trigger
										class="w-14 shrink-0 cursor-help text-left font-mono text-[10px] text-muted-foreground"
										data-testid="evescout-signature"
									>
										{c.hub_signature || '---'}
									</Tooltip.Trigger>
									<Tooltip.Content class="flex flex-col gap-0.5 text-xs">
										<span>{c.hub_signature || '?'} in {c.hub}</span>
										<span class="text-muted-foreground">
											{c.signature || '?'} on the far side
										</span>
										{#if c.max_ship_size}
											<span class="text-muted-foreground">Up to {c.max_ship_size}</span>
										{/if}
									</Tooltip.Content>
								</Tooltip.Root>

								<span class="w-10 shrink-0 font-mono text-[10px] text-muted-foreground">
									{c.wormhole_type ?? '---'}
								</span>

								<span class="w-10 shrink-0 text-right">
									{#if row.route}
										<RoutePopover {map} steps={row.route.route}>
											<span
												class={cn('cursor-pointer font-medium tabular-nums', jumpTone(row.jumps ?? 0))}
												data-testid="evescout-jumps">{row.jumps}j</span
											>
										</RoutePopover>
									{:else}
										<span class="text-[10px] whitespace-nowrap text-muted-foreground/60">--</span>
									{/if}
								</span>

								<span
									class={cn(
										'w-10 shrink-0 text-right font-mono text-[10px] font-semibold tabular-nums',
										ttlTone(c.remaining_hours)
									)}
									data-testid="evescout-ttl"
								>
									{ttl(c.remaining_hours)}
								</span>
							</div>
						{/snippet}

						{#if row.system}
							<SystemMenu system={row.system}>{@render line()}</SystemMenu>
						{:else}
							{@render line()}
						{/if}
					{/each}
				{/if}
			</div>
		</MapPanelContent>
	</MapPanel>
</Tooltip.Provider>
