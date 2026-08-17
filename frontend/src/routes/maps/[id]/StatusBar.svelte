<script lang="ts">
	// The map's chrome strip: identity on the left, warnings in the middle, and the
	// controls that act on the whole map on the right. Everything here is either about
	// the map as a whole or about the viewer, never about one system.
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import LayersIcon from '@lucide/svelte/icons/layers';
	import LayoutGridIcon from '@lucide/svelte/icons/layout-grid';
	import RadarIcon from '@lucide/svelte/icons/radar';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Redo2Icon from '@lucide/svelte/icons/redo-2';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import Undo2Icon from '@lucide/svelte/icons/undo-2';

	import { api } from '$lib/api/client';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Separator } from '$lib/components/ui/separator';
	import * as Popover from '$lib/components/ui/popover';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { classMeta } from '$lib/map/classes';
	import { cn } from '$lib/utils';
	import type { MapState } from './map-state.svelte';

	let { map }: { map: MapState } = $props();

	const canWrite = $derived(map.data?.role === 'member' || map.data?.role === 'owner');

	// Where the acting pilot is, resolved against the map's own systems so we can show the
	// class chip. A pilot outside the mapped chain still gets their system id.
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
		api
			.updateMapUserSettings(map.mapId, { [key]: !current[key] })
			.then((s) => {
				map.userSettings = s;
				if (key === 'tracking_allowed') map.fetchCharacters();
			})
			.catch(() => {});
	}

	function relative(iso: string): string {
		const secs = Math.round((Date.now() - new Date(iso).getTime()) / 1000);
		if (secs < 60) return 'just now';
		if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
		if (secs < 86_400) return `${Math.floor(secs / 3600)}h ago`;
		return `${Math.floor(secs / 86_400)}d ago`;
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
	<a
		href="/maps"
		class="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
	>
		<ArrowLeftIcon class="size-4" />
		Maps
	</a>
	<Separator orientation="vertical" class="h-4" />
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
		{#if map.data && !map.data.character_has_access}
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
								Clean map ({map.stale.length})
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
				{@const meta = classMeta(pilotSystem.wormhole_class_id, pilotSystem.security_status)}
				<span class={cn('font-mono', meta.token)}>{meta.short}</span>
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
					onclick={() => (map.editingLayout = !map.editingLayout)}
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
				<span class="ml-1 font-normal text-muted-foreground">
					newest first; struck through means undone
				</span>
			</div>
			{#if map.entries.length === 0}
				<p class="px-3 py-6 text-center text-xs text-muted-foreground">Nothing yet.</p>
			{:else}
				<ul class="max-h-80 overflow-y-auto py-1" data-testid="history-list">
					{#each map.entries as entry (entry.id)}
						{@const isHead = entry.id === map.history?.head_event_id}
						<li>
							<button
								type="button"
								class={cn(
									'flex w-full items-baseline gap-2 px-3 py-1.5 text-left text-xs',
									entry.is_step && canWrite && 'hover:bg-accent',
									!entry.is_step && 'cursor-default',
									entry.is_step && !entry.applied && 'text-muted-foreground line-through',
									isHead && 'bg-accent/50'
								)}
								data-testid="history-row"
								data-applied={entry.applied}
								data-head={isHead}
								disabled={!entry.is_step || !canWrite || isHead}
								title={entry.is_step
									? isHead
										? 'The map is here'
										: 'Move the map to this point'
									: 'Recorded automatically; not part of undo'}
								onclick={() => map.gotoEvent(entry.id)}
							>
								<span
									class={cn(
										'mt-1 size-1.5 shrink-0 rounded-full',
										isHead ? 'bg-amber-400' : entry.applied ? 'bg-border' : 'bg-transparent'
									)}
								></span>
								<span class="flex-1 truncate">
									<span class="text-muted-foreground">{entry.character_name ?? 'Vector'}</span>
									{entry.label}
								</span>
								<span class="shrink-0 text-muted-foreground">{relative(entry.created_at)}</span>
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
</div>
</Tooltip.Provider>
