<script lang="ts">
	// One placed system, rendered to legacy parity: class label, alias/name/occupier, an
	// icon cluster with tooltips, region or statics, and four styling channels (status
	// border, selected background, active ring, hover outline).
	import ActivityIcon from '@lucide/svelte/icons/activity';
	import ApertureIcon from '@lucide/svelte/icons/aperture';
	import CircleDashedIcon from '@lucide/svelte/icons/circle-dashed';
	import CircleHelpIcon from '@lucide/svelte/icons/circle-help';
	import FanIcon from '@lucide/svelte/icons/fan';
	import FlagIcon from '@lucide/svelte/icons/flag';
	import HomeIcon from '@lucide/svelte/icons/home';
	import LockIcon from '@lucide/svelte/icons/lock';
	import RadarIcon from '@lucide/svelte/icons/radar';
	import SatelliteIcon from '@lucide/svelte/icons/satellite';
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import SkullIcon from '@lucide/svelte/icons/skull';

	import type { MapCharacter } from '$lib/api/types/MapCharacter';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { SystemStatus } from '$lib/api/types/SystemStatus';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Popover from '$lib/components/ui/popover';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import EveImage from '$lib/components/EveImage.svelte';
	import StaticDetails from '$lib/components/map/StaticDetails.svelte';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import { classMeta, destClassMeta, isWormholeClass } from '$lib/map/classes';
	import { NODE_W, statusColor } from '$lib/map/helpers';
	import EffectBadge from '$lib/components/EffectBadge.svelte';

	export interface SigCounts {
		total: number;
		uncategorized: number;
		wormholes: number;
	}

	let {
		node,
		nodeH,
		selected,
		active = false,
		highlighted = false,
		pos,
		sigCounts,
		connectionCount,
		pilots = [],
		showThreat = true,
		onselect,
		ondown,
		onlink,
		onmenu,
		onsavealias,
		draggable = true
	}: {
		node: MapSystemView;
		nodeH: number;
		selected: boolean;
		active?: boolean;
		/** Pointed at from a side panel row, so it can be found without reading labels. */
		highlighted?: boolean;
		pos: { x: number; y: number };
		sigCounts: SigCounts;
		connectionCount: number;
		pilots?: MapCharacter[];
		showThreat?: boolean;
		onselect: (ev: PointerEvent) => void;
		ondown: (ev: PointerEvent) => void;
		onlink: (ev: PointerEvent) => void;
		/** An automatic layout owns the positions, so the drag handle goes away. */
		draggable?: boolean;
		onmenu: (ev: MouseEvent) => void;
		onsavealias: (alias: string | null, occupier: string | null) => void;
	} = $props();

	const cls = $derived(classMeta(node.wormhole_class_id, node.security_status));
	// Threat ring: high/critical only, user-toggleable, suppressed while active.
	const threatRing = $derived(
		showThreat && !active && (node.threat_level === 'high' || node.threat_level === 'critical')
			? node.threat_level
			: null
	);
	const showStatics = $derived(isWormholeClass(node.wormhole_class_id) && node.statics.length > 0);
	// A hole nobody has been through: drawn as a node so the chain can be laid out and
	// named, dashed so it never reads as somewhere you can actually go.
	const ghost = $derived(node.solar_system_id === null);
	const unmapped = $derived(Math.max(0, sigCounts.wormholes - connectionCount));
	// EVE's image server serves a faction's logo from the corporations endpoint keyed by
	// the faction id, so the faction id can be used directly.
	const sovKind = $derived(node.sovereignty?.kind === 'alliance' ? 'alliance' : 'corporation');

	const STATUS_ICONS: Record<SystemStatus, typeof ShieldCheckIcon> = {
		friendly: ShieldCheckIcon,
		hostile: SkullIcon,
		active: ActivityIcon,
		unscanned: RadarIcon,
		empty: CircleDashedIcon,
		unknown: CircleHelpIcon
	};
	const StatusIcon = $derived(STATUS_ICONS[node.status]);

	// Alias/occupier editor, opened by double-clicking the card.
	let editorOpen = $state(false);
	let editAlias = $state('');
	let editOccupier = $state('');

	function openEditor() {
		editAlias = node.alias ?? '';
		editOccupier = node.occupying_group ?? '';
		editorOpen = true;
	}

	function saveEditor() {
		onsavealias(editAlias.trim() || null, editOccupier.trim() || null);
		editorOpen = false;
	}
</script>

<Tooltip.Provider delayDuration={500}>
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		data-testid="system-node"
		data-status={node.status}
		data-threat={threatRing}
		data-ghost={ghost ? 'true' : undefined}
		class="group/node absolute flex flex-col justify-center rounded border bg-card px-2 py-0.5 text-[11px] leading-tight shadow-sm transition-colors duration-200
			{ghost ? 'border-dashed bg-card/60' : ''}
			{selected ? 'bg-amber-100 dark:bg-amber-900' : ''}
			{active ? 'z-10 ring-2 ring-amber-500 ring-offset-2 ring-offset-background' : ''}
			{highlighted ? 'z-20 outline-2 outline-yellow-500' : ''}
			{threatRing ? 'ring-2' : ''}
			hover:z-20 hover:outline-2 hover:outline-yellow-500"
		style:--tw-ring-color={threatRing ? `var(--color-threat-${threatRing})` : null}
		style:border-color={statusColor(node.status)}
		style:width="{NODE_W}px"
		style:height="{pilots.length > 0 ? nodeH + 20 : nodeH}px"
		style:left="{pos.x}px"
		style:top="{pos.y}px"
		onpointerdown={onselect}
		oncontextmenu={onmenu}
		ondblclick={openEditor}
	>
		<!-- Drag handle (top), hover-only, hidden when pinned or placed for you. -->
		{#if !node.is_pinned && draggable}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="absolute top-[1px] left-1/2 hidden h-2 w-12 -translate-x-1/2 -translate-y-1/2 cursor-move rounded border border-neutral-300 bg-white group-hover/node:z-50 group-hover/node:block dark:border-neutral-600 dark:bg-neutral-700"
				data-testid="drag-handle"
				onpointerdown={(ev) => {
					ev.stopPropagation();
					ondown(ev);
				}}
			></div>
		{/if}
		<!-- Connection handle (right edge), hover-only. -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="absolute top-1/2 left-full hidden h-4 w-4 -translate-x-1/2 -translate-y-1/2 cursor-pointer rounded-full border border-neutral-300 bg-white group-hover/node:block hover:block dark:border-neutral-600 dark:bg-neutral-700"
			data-testid="connection-handle"
			onpointerdown={onlink}
		></div>

		<!-- Row 1: class, alias/name/occupier, icon cluster. -->
		<div class="flex min-w-0 items-center gap-1">
			<ClassBadge
				classId={node.wormhole_class_id}
				security={node.security_status}
				class="shrink-0 font-medium"
			/>
			{#if node.alias}
				<span class="shrink-0 font-medium text-foreground">{node.alias}</span>
				<span class="truncate text-muted-foreground">{node.name ?? 'Unmapped'}</span>
			{:else if node.name}
				<span class="truncate font-medium text-foreground">{node.name}</span>
			{:else}
				<span class="truncate font-medium text-muted-foreground italic">Unmapped</span>
			{/if}
			{#if node.occupying_group}
				<span class="shrink-0 text-muted-foreground">({node.occupying_group})</span>
			{/if}

			<span class="ml-auto flex shrink-0 items-center gap-1">
				{#if node.status !== 'unknown'}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex" aria-label={node.status}>
							<StatusIcon class="size-[14px]" style="color: {statusColor(node.status)}" />
						</Tooltip.Trigger>
						<Tooltip.Content>{node.status[0].toUpperCase() + node.status.slice(1)}</Tooltip.Content>
					</Tooltip.Root>
				{/if}
				{#if node.is_home}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex"><HomeIcon class="size-[14px] text-amber-400" /></Tooltip.Trigger>
						<Tooltip.Content>Home system</Tooltip.Content>
					</Tooltip.Root>
				{/if}
				{#if node.is_rally}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex"><FlagIcon class="size-[14px] text-red-400" /></Tooltip.Trigger>
						<Tooltip.Content>Rally point</Tooltip.Content>
					</Tooltip.Root>
				{/if}
				{#if node.is_pinned}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex">
							<LockIcon class="size-[14px] text-muted-foreground" />
						</Tooltip.Trigger>
						<Tooltip.Content>Pinned in place</Tooltip.Content>
					</Tooltip.Root>
				{/if}
				{#if sigCounts.total > 0}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex" data-testid="sig-icon">
							<SatelliteIcon
								class="size-[14px] {sigCounts.uncategorized > 0 ? 'text-rose-500' : 'text-amber-500'}"
							/>
						</Tooltip.Trigger>
						<Tooltip.Content>
							{sigCounts.total} signature{sigCounts.total === 1 ? '' : 's'}{sigCounts.uncategorized > 0
								? `, ${sigCounts.uncategorized} uncategorized`
								: ''}
						</Tooltip.Content>
					</Tooltip.Root>
				{/if}
				{#if unmapped > 0}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex" data-testid="unmapped-icon">
							<FanIcon class="size-[14px] text-sky-500" />
						</Tooltip.Trigger>
						<Tooltip.Content>
							Has {unmapped} unmapped wormhole{unmapped === 1 ? '' : 's'}
						</Tooltip.Content>
					</Tooltip.Root>
				{/if}
				{#if node.is_shattered}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex" data-testid="shattered-icon">
							<ApertureIcon class="size-3 text-amber-500/90" />
						</Tooltip.Trigger>
						<Tooltip.Content>Shattered system</Tooltip.Content>
					</Tooltip.Root>
				{/if}
				{#if node.sovereignty}
					<Tooltip.Root>
						<Tooltip.Trigger class="flex">
							<EveImage kind={sovKind} id={node.sovereignty.id} class="size-4 shrink-0 rounded-sm" />
						</Tooltip.Trigger>
						<Tooltip.Content class="flex items-center gap-2">
							<EveImage kind={sovKind} id={node.sovereignty.id} class="size-6 rounded-sm" />
							{node.sovereignty.name}
							{#if 'ticker' in node.sovereignty}({node.sovereignty.ticker}){/if}
						</Tooltip.Content>
					</Tooltip.Root>
				{:else if node.effect_name}
					<EffectBadge name={node.effect_name} wormholeClassId={node.wormhole_class_id ?? 0} />
				{/if}
			</span>
		</div>

		<!-- Row 2: region for k-space, statics for w-space. -->
		<div class="flex items-center gap-1.5 text-[10px]">
			{#if showStatics}
				{#each node.statics as st (st.code)}
					{@const dest = destClassMeta(st.dest_class)}
					<Tooltip.Root delayDuration={700}>
						<Tooltip.Trigger
							class="flex font-medium"
							data-testid="static-badge"
							style="color: var(--color-{dest.token})"
						>
							{dest.short}
						</Tooltip.Trigger>
						<Tooltip.Content class="p-0" side="bottom">
							<StaticDetails static={st} />
						</Tooltip.Content>
					</Tooltip.Root>
				{/each}
			{:else}
				<span class="truncate text-muted-foreground">{node.region}</span>
			{/if}
		</div>

		{#if pilots.length > 0}
			<Tooltip.Root delayDuration={700}>
				<Tooltip.Trigger
					class="mt-0.5 flex h-[18px] items-center gap-1.5 border-t border-border pt-0.5 text-[10px]"
					data-testid="pilots-row"
				>
					<span class="size-1 animate-pulse rounded-full bg-green-500"></span>
					<span class="truncate">{pilots[0].name}</span>
					{#if pilots.length > 1}
						<span class="shrink-0 text-muted-foreground">and {pilots.length - 1} more</span>
					{/if}
				</Tooltip.Trigger>
				<Tooltip.Content class="p-2" side="bottom">
					<div class="flex max-h-64 flex-col gap-1 overflow-auto">
						{#each pilots as p (p.character_id)}
							<div class="flex items-center gap-2 text-[11px]">
								<EveImage kind="character" id={p.character_id} class="size-5 rounded-full" />
								{p.name}
								<span class="text-muted-foreground">[{p.corporation_ticker}]</span>
								{#if p.ship_type}
									<span class="ml-auto text-muted-foreground">{p.ship_type}</span>
								{/if}
							</div>
						{/each}
					</div>
				</Tooltip.Content>
			</Tooltip.Root>
		{/if}

		<!-- Alias/occupier editor (double click). -->
		<Popover.Root bind:open={editorOpen}>
			<Popover.Trigger class="pointer-events-none absolute inset-x-0 top-0" tabindex={-1} />
			<Popover.Content
				class="flex w-56 flex-col gap-2 p-2"
				onpointerdown={(ev: PointerEvent) => ev.stopPropagation()}
				ondblclick={(ev: MouseEvent) => ev.stopPropagation()}
			>
				<Input
					placeholder="Alias"
					bind:value={editAlias}
					onkeydown={(ev) => ev.key === 'Enter' && saveEditor()}
				/>
				{#if !ghost}
					<Input
						placeholder="Occupier alias"
						bind:value={editOccupier}
						onkeydown={(ev) => ev.key === 'Enter' && saveEditor()}
					/>
				{/if}
				<Button size="sm" onclick={saveEditor}>Save</Button>
			</Popover.Content>
		</Popover.Root>
	</div>
</Tooltip.Provider>
