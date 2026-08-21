<script lang="ts">
	// What has died in the chain lately: whether anything is hunting here, and whether it was
	// worth anything.
	import FilterIcon from '@lucide/svelte/icons/list-filter';
	import { solarSystemId } from '$lib/map/system';

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

	let { map }: { map: MapState } =
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
		// The list itself arrives by push; this is only the clock.
		const clock = setInterval(() => (now = new Date()), 30_000);
		return () => clearInterval(clock);
	});

	// A value rather than an array identity: the graph is refetched constantly, and a new
	// array each time would refetch kills on every one of them.
	const systemKey = $derived(
		map.systems
			.map(solarSystemId)
			.filter((id) => id !== null)
			.sort((a, b) => a - b)
			.join(',')
	);

	$effect(() => {
		// The list is scoped to the map's systems, so adding one is as much a change as a fresh
		// kill arriving. Read here to register the dependency.
		filter;
		systemKey;
		map.killmailTick;
		load();
	});

	function setFilter(value: string) {
		map.patchUserSettings({ killmail_filter: value }).catch(() => {});
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
		return map.systems.find((s) => solarSystemId(s) === kill.solar_system_id)?.alias ?? null;
	}

	/** An NPC kill in the chain is noise; a solo kill is a hunter. */
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

	/** Who they fly for, spelled out. The row has room for a ticker at most. */
	function partyOrg(party: MapKillmail['victim']): string | null {
		const corp = party.corporation_name;
		const alliance = party.alliance_name;
		if (corp && alliance) return `${corp} · ${alliance}`;
		return alliance ?? corp ?? null;
	}

	function hover(kill: MapKillmail, on: boolean) {
		const placed = map.systems.find((s) => solarSystemId(s) === kill.solar_system_id);
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
				<div class="@container flex flex-col">
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
										{#if partyOrg(kill.victim)}
											<span class="text-muted-foreground">{partyOrg(kill.victim)}</span>
										{/if}
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

								<!-- Who did it. The icon and the count are min-content and left-aligned, so
								     every row lines up on the icon rather than on a number whose width
								     depends on how many attackers there were. -->
								<Tooltip.Root>
									<Tooltip.Trigger
										class="flex min-w-0 shrink-0 items-center gap-1.5 @min-[620px]:w-40 @min-[760px]:w-56"
									>
										{#if kill.final_blow.ship_type_id}
											<EveImage
												kind="type"
												id={kill.final_blow.ship_type_id}
												class="size-4 shrink-0 rounded"
											/>
										{/if}
										<span
											class={cn('shrink-0 font-mono text-[10px] tabular-nums', crowdTone(kill))}
											data-testid="killmail-attackers"
										>
											{kill.attacker_count}
										</span>
										<!-- Only where the card is wide enough to say it without crowding. -->
										<span
											class="hidden min-w-0 truncate text-left text-[10px] text-muted-foreground @min-[620px]:inline"
											data-testid="killmail-aggressor"
										>
											{kill.final_blow.character_name ?? (kill.is_npc ? 'NPCs' : 'Unknown')}
										</span>
										<span
											class="hidden min-w-0 truncate text-left font-mono text-[10px] text-muted-foreground/60 @min-[760px]:inline"
											data-testid="killmail-aggressor-org"
										>
											{kill.final_blow.alliance_ticker ?? kill.final_blow.corporation_ticker ?? ''}
										</span>
									</Tooltip.Trigger>
									<Tooltip.Content class="flex flex-col gap-0.5">
										<span>{crowdLabel(kill)}</span>
										{#if !kill.is_npc}
											<span class="text-muted-foreground">
												Final blow: {partyName(kill.final_blow)}
												{#if kill.final_blow.ship_name}({kill.final_blow.ship_name}){/if}
											</span>
											{#if partyOrg(kill.final_blow)}
												<span class="text-muted-foreground">{partyOrg(kill.final_blow)}</span>
											{/if}
										{/if}
									</Tooltip.Content>
								</Tooltip.Root>

								<a
									class={cn(
										'hidden w-14 shrink-0 text-right font-mono text-[10px] tabular-nums hover:underline @min-[380px]:inline',
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
									class="hidden w-14 shrink-0 text-right font-mono text-[10px] whitespace-nowrap text-muted-foreground/60 @min-[480px]:inline"
									data-testid="killmail-age"
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
