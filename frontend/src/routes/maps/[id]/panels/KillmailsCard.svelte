<script lang="ts">
	// What has died in the chain lately.
	//
	// The list answers two questions at a glance: is anything hunting here, and was it
	// worth anything. So the ISK column is the one that shouts, and the attacker count
	// carries the shape of the fight (solo, blob, or just rats).
	import FilterIcon from '@lucide/svelte/icons/list-filter';

	import { api } from '$lib/api/client';
	import type { MapKillmail } from '$lib/api/types/MapKillmail';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import EveImage from '$lib/components/EveImage.svelte';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { formatIsk, iskTone, timeAgo } from '$lib/format';
	import { cn } from '$lib/utils';
	import type { MapState } from '../map-state.svelte';

	let { map, layoutActions }: { map: MapState; layoutActions?: import('svelte').Snippet } =
		$props();

	let kills = $state<MapKillmail[]>([]);
	let now = $state(new Date());

	const filter = $derived(map.userSettings?.killmail_filter ?? 'all');
	const FILTERS = [
		{ value: 'all', label: 'Everywhere on the map' },
		{ value: 'jspace', label: 'Wormhole space only' },
		{ value: 'kspace', label: 'Known space only' }
	];

	function load() {
		api
			.mapKillmails(map.mapId)
			.then((rows) => (kills = rows))
			.catch(() => {});
	}

	$effect(() => {
		// Timers are read to the minute; the list itself arrives by push.
		const clock = setInterval(() => (now = new Date()), 30_000);
		return () => clearInterval(clock);
	});

	// A kill anywhere on the map bumps the tick; refetch rather than splice, because what
	// belongs in the list depends on this viewer's own filter.
	// The set of systems the list covers, as a value rather than an array identity: the
	// graph is refetched constantly and a new array each time would mean refetching kills
	// on every one of them.
	const systemKey = $derived(
		map.systems
			.map((s) => s.solar_system_id)
			.sort((a, b) => a - b)
			.join(',')
	);

	$effect(() => {
		// The list is scoped to the map's systems, so adding one is as much a change as a
		// fresh kill arriving or the filter moving.
		filter;
		systemKey;
		map.killmailTick;
		load();
	});

	function setFilter(value: string) {
		api
			.updateMapUserSettings(map.mapId, { killmail_filter: value })
			.then((s) => (map.userSettings = s))
			.catch(() => {});
	}

	/** The row's system in the shape the context menu wants, from the payload. */
	function systemOf(kill: MapKillmail): SystemSearchResult {
		return {
			id: kill.solar_system_id,
			name: kill.system_name,
			security: kill.security_status,
			region: kill.region,
			// Only needed by the menu's zKillboard links, which the row does not offer.
			region_id: 0,
			constellation_id: 0,
			wormhole_class_id: kill.wormhole_class_id ?? null,
			effect_name: null,
			sovereignty: null,
			// The row names a system it already knows; statics are not part of a kill.
			statics: []
		};
	}

	/** The map's own name for the system, when it has one. */
	function aliasOf(kill: MapKillmail): string | null {
		return map.systems.find((s) => s.solar_system_id === kill.solar_system_id)?.alias ?? null;
	}

	/**
	 * What the attacker count means, not just how many. An NPC kill in your chain is
	 * noise; a solo kill is a hunter.
	 */
	function crowdTone(kill: MapKillmail): string {
		if (kill.is_npc) return 'text-muted-foreground/50';
		if (kill.is_solo) return 'text-amber-400';
		return 'text-muted-foreground';
	}

	function crowdLabel(kill: MapKillmail): string {
		if (kill.is_npc) return 'Killed by NPCs';
		if (kill.is_solo) return 'Solo kill';
		return `${kill.attacker_count} attackers`;
	}

	function partyName(party: MapKillmail['victim']): string {
		const ticker = party.alliance_ticker ?? party.corporation_ticker;
		const who = party.character_name ?? 'Unknown pilot';
		return ticker ? `${who} [${ticker}]` : who;
	}

	function hover(kill: MapKillmail, on: boolean) {
		const placed = map.systems.find((s) => s.solar_system_id === kill.solar_system_id);
		map.hoveredSystemId = on ? (placed?.id ?? null) : null;
	}

	const zkill = (id: number) => `https://zkillboard.com/kill/${id}/`;
</script>

<Tooltip.Provider delayDuration={300}>
	<MapPanel testid="killmails-card">
		<MapPanelHeader>
			<span class="inline-flex items-center gap-2">
				Killmails
				{#if kills.length > 0}
					<span class="font-mono text-amber-400">{kills.length}</span>
				{/if}
			</span>
			{#snippet actions()}
				{@render layoutActions?.()}
				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="ghost"
								size="icon"
								class="size-6"
								aria-label="Filter killmails"
								data-testid="killmail-filter"
							>
								<FilterIcon />
							</Button>
						{/snippet}
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="end">
						<DropdownMenu.Group>
							{#each FILTERS as option (option.value)}
								<DropdownMenu.CheckboxItem
									checked={filter === option.value}
									onCheckedChange={() => setFilter(option.value)}
									data-testid="killmail-filter-{option.value}"
								>
									{option.label}
								</DropdownMenu.CheckboxItem>
							{/each}
						</DropdownMenu.Group>
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			{/snippet}
		</MapPanelHeader>

		<MapPanelContent>
			{#if kills.length === 0}
				<p class="px-3 py-4 text-xs text-muted-foreground" data-testid="killmails-empty">
					Nothing has died in these systems in the last week.
				</p>
			{:else}
				<div class="flex flex-col">
					{#each kills as kill (kill.id)}
						{@const alias = aliasOf(kill)}
						{#snippet row()}
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								class="flex items-center gap-2 border-b border-border/30 px-3 py-1 text-xs last:border-b-0 hover:bg-muted/30"
								data-testid="killmail-row"
								data-kill={kill.id}
								onmouseenter={() => hover(kill, true)}
								onmouseleave={() => hover(kill, false)}
							>
								<!-- What died, and who was flying it. -->
								<Tooltip.Root>
									<Tooltip.Trigger class="flex shrink-0 items-center gap-1">
										{#if kill.victim.ship_type_id}
											<EveImage kind="type" id={kill.victim.ship_type_id} class="size-5 rounded" />
										{/if}
										{#if kill.victim.character_id}
											<EveImage
												kind="character"
												id={kill.victim.character_id}
												class="size-5 rounded"
											/>
										{/if}
									</Tooltip.Trigger>
									<Tooltip.Content class="flex flex-col gap-0.5">
										<span>{partyName(kill.victim)}</span>
										<span class="text-muted-foreground">
											lost {kill.victim.ship_name ?? 'a ship'}
										</span>
									</Tooltip.Content>
								</Tooltip.Root>

								<span class="min-w-0 flex-1 truncate" title={kill.victim.ship_name ?? ''}>
									{kill.victim.ship_name ?? 'Unknown ship'}
									{#if kill.victim.alliance_ticker ?? kill.victim.corporation_ticker}
										<span class="font-mono text-[10px] text-muted-foreground">
											[{kill.victim.alliance_ticker ?? kill.victim.corporation_ticker}]
										</span>
									{/if}
								</span>

								<span class="flex min-w-0 flex-1 items-center gap-1.5">
									<ClassBadge
										classId={kill.wormhole_class_id === null ? null : Number(kill.wormhole_class_id)}
										security={kill.security_status}
										class="shrink-0 text-[10px]"
									/>
									<span class="min-w-0 truncate" title="{kill.system_name} · {kill.region}">
										{#if alias}<span class="font-medium">{alias}</span> · {/if}{kill.system_name}
									</span>
								</span>

								<!-- Who landed the blow, and how many were on it. -->
								<Tooltip.Root>
									<Tooltip.Trigger class="flex w-12 shrink-0 items-center justify-end gap-1">
										{#if kill.final_blow.ship_type_id}
											<EveImage
												kind="type"
												id={kill.final_blow.ship_type_id}
												class="size-4 shrink-0 rounded"
											/>
										{/if}
										<span
											class={cn('font-mono text-[10px] tabular-nums', crowdTone(kill))}
											data-testid="killmail-attackers"
										>
											{kill.attacker_count}
										</span>
									</Tooltip.Trigger>
									<Tooltip.Content class="flex flex-col gap-0.5">
										<span>{crowdLabel(kill)}</span>
										{#if !kill.is_npc}
											<span class="text-muted-foreground">
												Final blow: {partyName(kill.final_blow)}
												{#if kill.final_blow.ship_name}({kill.final_blow.ship_name}){/if}
											</span>
										{/if}
									</Tooltip.Content>
								</Tooltip.Root>

								<a
									class={cn(
										'w-14 shrink-0 text-right font-mono text-[10px] tabular-nums hover:underline',
										iskTone(kill.total_value)
									)}
									href={zkill(kill.id)}
									target="_blank"
									rel="noopener"
									data-testid="killmail-value"
									title="Open on zKillboard"
								>
									{formatIsk(kill.total_value) ?? '--'}
								</a>

								<span
									class="w-14 shrink-0 text-right font-mono text-[10px] whitespace-nowrap text-muted-foreground/60"
								>
									{timeAgo(kill.time, now)}
								</span>
							</div>
						{/snippet}

						<SystemMenu system={systemOf(kill)}>{@render row()}</SystemMenu>
					{/each}
				</div>
			{/if}
		</MapPanelContent>
	</MapPanel>
</Tooltip.Provider>
