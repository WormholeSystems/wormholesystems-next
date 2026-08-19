<script lang="ts">
	// The map's chrome strip. Everything here is about the map as a whole or about the viewer,
	// never about one system.
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import LayersIcon from '@lucide/svelte/icons/layers';
	import LayoutGridIcon from '@lucide/svelte/icons/layout-grid';
	import RadarIcon from '@lucide/svelte/icons/radar';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Redo2Icon from '@lucide/svelte/icons/redo-2';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import BrushCleaningIcon from '@lucide/svelte/icons/brush-cleaning';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import Undo2Icon from '@lucide/svelte/icons/undo-2';

	import { page } from '$app/state';

	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Separator } from '$lib/components/ui/separator';
	import * as Popover from '$lib/components/ui/popover';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import { historyRows } from './history-tree';
	import { cn } from '$lib/utils';
	import { timeAgo } from '$lib/format';
	import type { MapState } from './map-state.svelte';
	import TrackingSettings from './TrackingSettings.svelte';
	import { atLeast } from '$lib/map/roles';

	let { map }: { map: MapState } = $props();

	const canWrite = $derived(atLeast(map.data?.role, 'member'));
	// Somebody following a share link has no pilot, so the pilot warnings have nobody to be
	// about.
	const watching = $derived(page.data.me == null);
	const rows = $derived(historyRows(map.entries));

	// The trunk runs oldest-first, so the map's position is near the bottom of a long history.
	// Binding the marker fires this once the popover's rows are in the DOM, with no timer.
	let headLabel = $state<HTMLElement | null>(null);
	$effect(() => {
		headLabel?.scrollIntoView({ block: 'nearest' });
	});

	// Resolved against the map's own systems for the class chip; a pilot outside the chain
	// still gets their system id.
	const pilot = $derived(map.myCharacters.find((c) => c.is_active) ?? null);
	const pilotSystem = $derived(
		map.systems.find((s) => s.solar_system_id === pilot?.solar_system_id) ?? null
	);

	const socketLabel: Record<typeof map.socket, string> = {
		connecting: 'Connecting to the live feed',
		open: 'Live: changes from other pilots arrive automatically',
		reconnecting: 'Disconnected. Retrying, the map may be out of date'
	};

	function toggleSetting(key: 'tracking_allowed' | 'show_threat_level' | 'show_statics_first') {
		const current = map.userSettings;
		if (!current) return;
		map
			.patchUserSettings({ [key]: !current[key] })
			.then(() => {
				if (key === 'tracking_allowed') map.fetchCharacters();
			})
			.catch(() => {});
	}

</script>

{#snippet toggle(
	label: string,
	on: boolean,
	Icon: typeof EyeIcon,
	key: 'tracking_allowed' | 'show_threat_level' | 'show_statics_first',
	testid: string
)}
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="ghost"
					size="icon"
					class={cn('size-7', on ? 'text-foreground' : 'text-muted-foreground/50')}
					aria-pressed={on}
					data-testid={testid}
					onclick={() => toggleSetting(key)}
				>
					<Icon />
				</Button>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content>{label}: {on ? 'on' : 'off'}</Tooltip.Content>
	</Tooltip.Root>
{/snippet}

<Tooltip.Provider delayDuration={300}>
<div
	class="flex h-10 items-center gap-2 border-b border-border/50 bg-muted/30 px-3"
	data-testid="status-bar"
>
	<!-- A watcher has no map list, no saved arrangement and no settings: the chain is the
	     whole page for them. -->
	{#if !watching}
		<a
			href="/maps"
			class="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
		>
			<ArrowLeftIcon class="size-4" />
			Maps
		</a>
		<Separator orientation="vertical" class="h-4" />
	{/if}
	<span class="truncate text-sm font-medium" data-testid="status-bar-name">
		{map.data?.map.name ?? '...'}
	</span>
	<Button
		variant="outline"
		size="sm"
		class="h-7 gap-2 text-muted-foreground"
		data-testid="palette-trigger"
		onclick={() => (map.paletteOpen = true)}
	>
		<SearchIcon />
		Search
		<kbd class="rounded border border-border/60 px-1 font-mono text-[10px]">⌘K</kbd>
	</Button>

	<div class="flex flex-1 items-center justify-center gap-2">
		{#if watching}
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<Badge {...props} variant="outline" class="gap-1 text-muted-foreground">
							<EyeIcon />
							Watching
						</Badge>
					{/snippet}
				</Tooltip.Trigger>
				<Tooltip.Content class="max-w-64">
					You are following a link to this map. It updates as the chain is scanned, but
					nothing here can be changed without an account that has access to it.
				</Tooltip.Content>
			</Tooltip.Root>
		{:else if map.data && !map.data.character_has_access}
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<Badge {...props} variant="outline" class="gap-1 border-amber-600/40 text-amber-500">
							<TriangleAlertIcon />
							Limited access
						</Badge>
					{/snippet}
				</Tooltip.Trigger>
				<Tooltip.Content class="max-w-64">
					You can see this map through another of your characters, but {pilot?.name ??
						'the active character'} has no access of its own. Location sharing and waypoints will not work
					for them.
				</Tooltip.Content>
			</Tooltip.Root>
		{/if}

		{#if map.orphaned.length > 0 && canWrite}
			<!-- What a collapsed hole leaves behind: a branch nothing reaches any more. -->
			<Badge
				variant="outline"
				class="cursor-pointer gap-1 border-amber-500/40 text-amber-500"
				data-testid="orphaned-badge"
				onclick={() => (map.cleanPrompt = true)}
			>
				<BrushCleaningIcon />
				{map.orphaned.length} adrift
			</Badge>
		{/if}

		{#if map.stale.length > 0}
			<Popover.Root>
				<Popover.Trigger>
					{#snippet child({ props })}
						<Badge
							{...props}
							variant="outline"
							class="cursor-pointer gap-1 border-red-600/40 text-red-500"
							data-testid="stale-badge"
						>
							<TriangleAlertIcon />
							{map.stale.length} stale
						</Badge>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content class="w-80 p-0" align="center">
					<div class="border-b border-border/50 px-3 py-2 text-xs">
						Critical for over an hour, so probably long gone.
					</div>
					<ul class="max-h-64 overflow-y-auto py-1" data-testid="stale-list">
						{#each map.stale as s (s.connection_id)}
							<li class="px-3 py-1 text-xs">
								{s.from_name}
								<span class="text-muted-foreground">to</span>
								{s.to_name}
							</li>
						{/each}
					</ul>
					{#if canWrite}
						<div class="border-t border-border/50 p-2">
							<Button
								variant="destructive"
								size="sm"
								class="w-full"
								data-testid="clean-stale"
								onclick={() => map.cleanStale()}
							>
								Remove {map.stale.length === 1 ? 'it' : 'them'}
							</Button>
						</div>
					{/if}
				</Popover.Content>
			</Popover.Root>
		{/if}
	</div>

	{#if pilot?.online && pilot.solar_system_id !== null}
		<span class="hidden items-center gap-1.5 text-xs text-muted-foreground lg:flex">
			{#if pilotSystem}
				<ClassBadge
					classId={pilotSystem.wormhole_class_id}
					security={pilotSystem.security_status}
				/>
				<span class="text-foreground">{pilotSystem.name}</span>
			{:else}
				<span>Outside the chain</span>
			{/if}
		</span>
	{/if}

	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<span
					{...props}
					data-testid="socket-dot"
					data-state={map.socket}
					class={cn(
						'size-2 rounded-full',
						map.socket === 'open' && 'bg-emerald-500',
						map.socket === 'connecting' && 'bg-amber-500',
						map.socket === 'reconnecting' && 'animate-pulse bg-red-500'
					)}
				></span>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content>{socketLabel[map.socket]}</Tooltip.Content>
	</Tooltip.Root>

	<Separator orientation="vertical" class="h-4" />

	{#if map.userSettings}
		{@render toggle(
			'Share location',
			map.userSettings.tracking_allowed,
			EyeIcon,
			'tracking_allowed',
			'tracking-toggle'
		)}
		<TrackingSettings {map} />
		{@render toggle(
			'Threat rings',
			map.userSettings.show_threat_level,
			RadarIcon,
			'show_threat_level',
			'threat-toggle'
		)}
		{@render toggle(
			'Statics first',
			map.userSettings.show_statics_first,
			LayersIcon,
			'show_statics_first',
			'statics-first-toggle'
		)}
	{/if}

	{#if canWrite}
		<Separator orientation="vertical" class="h-4" />
		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						variant="ghost"
						size="icon"
						class="size-7"
						data-testid="undo-button"
						disabled={!map.canUndo}
						onclick={() => map.undo()}
					>
						<Undo2Icon />
					</Button>
				{/snippet}
			</Tooltip.Trigger>
			<Tooltip.Content>
				{map.headEntry ? `Undo: ${map.headEntry.label}` : 'Nothing to undo'}
			</Tooltip.Content>
		</Tooltip.Root>
		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						variant="ghost"
						size="icon"
						class="size-7"
						data-testid="redo-button"
						disabled={!map.canRedo}
						onclick={() => map.redo()}
					>
						<Redo2Icon />
					</Button>
				{/snippet}
			</Tooltip.Trigger>
			<Tooltip.Content>
				{map.redoEntry ? `Redo: ${map.redoEntry.label}` : 'Nothing to redo'}
			</Tooltip.Content>
		</Tooltip.Root>
	{/if}

	{#if !watching}
		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						variant="ghost"
						size="icon"
						class={cn('size-7', map.editingLayout && 'bg-accent text-foreground')}
						aria-pressed={map.editingLayout}
						data-testid="layout-toggle"
						onclick={() =>
							map.editingLayout ? map.exitLayoutEdit() : (map.editingLayout = true)}
					>
						<LayoutGridIcon />
					</Button>
				{/snippet}
			</Tooltip.Trigger>
			<Tooltip.Content>
				{map.editingLayout ? 'Done arranging panels' : 'Arrange the side panels'}
			</Tooltip.Content>
		</Tooltip.Root>

		<Popover.Root>
			<Popover.Trigger>
				{#snippet child({ props })}
					<Button {...props} variant="ghost" size="icon" class="size-7" data-testid="history-button">
						<HistoryIcon />
					</Button>
				{/snippet}
			</Popover.Trigger>
			<Popover.Content class="w-96 p-0" align="end">
				<div class="border-b border-border/50 px-3 py-2 text-xs font-medium">
					History
					<span class="ml-1 font-normal text-muted-foreground">newest first</span>
				</div>
				{#if rows.length === 0}
					<p class="px-3 py-6 text-center text-xs text-muted-foreground">Nothing yet.</p>
				{:else}
					<ul class="max-h-80 overflow-y-auto py-1" data-testid="history-list">
						{#each rows as row (row.entry.id)}
							{@const entry = row.entry}
							{@const isHead = entry.id === map.history?.head_event_id}
							{@const navigable = entry.is_step && canWrite && !isHead}
							<li>
								<button
									type="button"
									class={cn(
										'flex w-full items-stretch gap-0 text-left text-xs',
										navigable && 'hover:bg-accent',
										!navigable && 'cursor-default',
										isHead && 'bg-accent/60'
									)}
									data-testid="history-row"
									data-applied={entry.applied}
									data-depth={row.depth}
									data-forks={row.forks}
									data-head={isHead}
									disabled={!navigable}
									title={entry.is_step
										? isHead
											? 'The map is here'
											: entry.applied
												? 'Rewind the map to this point'
												: 'Return to this branch'
										: 'Recorded automatically; not part of undo'}
									onclick={() => map.gotoEvent(entry.id)}
								>
									<!-- A rail for each line still open above this row, then this row's own
									     dot. Every line is centred in a 16px cell, so a branch's connector
									     meets the rail it left exactly. -->
									{#each row.rails as passing, i (i)}
										<span class="relative w-4 shrink-0">
											{#if passing}
												<span
													class="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-foreground/25"
												></span>
											{/if}
										</span>
									{/each}
									<span class="relative w-4 shrink-0">
										{#if row.railUp}
											<span
												class="absolute top-0 bottom-1/2 left-1/2 w-px -translate-x-1/2 bg-foreground/25"
											></span>
										{/if}
										{#if row.railDown}
											<span
												class="absolute top-1/2 bottom-0 left-1/2 w-px -translate-x-1/2 bg-foreground/25"
											></span>
										{/if}
										{#if row.forks}
											<!-- Where this line left the one it branched from. -->
											<span
												class="absolute top-1/2 right-1/2 h-px w-4 -translate-y-1/2 bg-foreground/25"
											></span>
										{/if}
										<span
											class={cn(
												'absolute top-1/2 left-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-popover',
												isHead
													? 'bg-amber-400'
													: !entry.is_step
														? 'bg-transparent ring-0'
														: entry.applied
															? 'bg-foreground/60'
															: 'bg-muted-foreground/40'
											)}
										></span>
									</span>
									<span class="flex flex-1 items-baseline gap-2 py-1.5 pr-3 min-w-0">
										<span
											class={cn(
												'flex-1 truncate',
												!entry.is_step && 'text-muted-foreground italic',
												entry.is_step && !entry.applied && 'text-muted-foreground'
											)}
										>
											<span class="text-muted-foreground">{entry.character_name ?? 'Vector'}</span>
											{entry.label}
										</span>
										{#if isHead}
											<span
												bind:this={headLabel}
												class="shrink-0 font-mono text-[10px] tracking-wider text-amber-400 uppercase"
											>
												here
											</span>
										{:else}
											<span class="shrink-0 text-muted-foreground">{timeAgo(entry.created_at)}</span>
										{/if}
									</span>
								</button>
							</li>
						{/each}
					</ul>
					{#if canWrite && map.history?.head_event_id != null}
						<div class="border-t border-border/50 p-2">
							<Button
								variant="ghost"
								size="sm"
								class="w-full text-xs"
								data-testid="history-rewind"
								onclick={() => map.gotoEvent(null)}
							>
								Rewind to the start
							</Button>
						</div>
					{/if}
				{/if}
			</Popover.Content>
		</Popover.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						href="/maps/{map.mapId}/settings"
						variant="ghost"
						size="icon"
						class="size-7"
						data-testid="settings-link"
					>
						<SettingsIcon />
					</Button>
				{/snippet}
			</Tooltip.Trigger>
			<Tooltip.Content>Map settings</Tooltip.Content>
		</Tooltip.Root>
	{/if}
</div>
</Tooltip.Provider>
