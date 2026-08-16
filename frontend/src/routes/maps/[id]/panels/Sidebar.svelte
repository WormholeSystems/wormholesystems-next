<script lang="ts">
	// The map's side column. Which panels appear, and in what order, is per user per map;
	// edit mode turns each panel header into a row of move/hide controls instead of
	// introducing a separate drag surface over a stack that is deliberately flush.
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import ChevronUpIcon from '@lucide/svelte/icons/chevron-up';
	import EyeOffIcon from '@lucide/svelte/icons/eye-off';
	import PlusIcon from '@lucide/svelte/icons/plus';

	import { api } from '$lib/api/client';
	import { Button } from '$lib/components/ui/button';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import NavigationCard from './NavigationCard.svelte';
	import NotesCard from './NotesCard.svelte';
	import SystemInfoCard from './SystemInfoCard.svelte';
	import ThreatCard from './ThreatCard.svelte';
	import SignaturesPanel from '../SignaturesPanel.svelte';
	import { PANELS, type PanelId, reorder, visiblePanels } from './layout';
	import type { MapState } from '../map-state.svelte';

	let { map }: { map: MapState } = $props();

	const order = $derived(map.userSettings?.panel_order ?? []);
	const hidden = $derived(map.userSettings?.hidden_panels ?? []);
	const panels = $derived(visiblePanels(order, hidden));
	const hiddenPanels = $derived(PANELS.filter((p) => hidden.includes(p.id)));

	function save(update: { hidden_panels?: string[]; panel_order?: string[] }) {
		api
			.updateMapUserSettings(map.mapId, update)
			.then((s) => (map.userSettings = s))
			.catch(() => {});
	}

	function move(id: PanelId, delta: -1 | 1) {
		save({ panel_order: reorder(order, hidden, id, delta) });
	}

	function hide(id: PanelId) {
		save({ hidden_panels: [...hidden, id] });
	}

	function show(id: PanelId) {
		save({ hidden_panels: hidden.filter((p) => p !== id) });
	}
</script>

{#snippet controls(id: PanelId, index: number)}
	{#if map.editingLayout}
		<span class="flex items-center gap-0.5" data-testid="panel-controls-{id}">
			<Button
				variant="ghost"
				size="icon"
				class="size-6"
				disabled={index === 0}
				aria-label="Move up"
				onclick={() => move(id, -1)}
			>
				<ChevronUpIcon />
			</Button>
			<Button
				variant="ghost"
				size="icon"
				class="size-6"
				disabled={index === panels.length - 1}
				aria-label="Move down"
				onclick={() => move(id, 1)}
			>
				<ChevronDownIcon />
			</Button>
			<Button
				variant="ghost"
				size="icon"
				class="size-6"
				aria-label="Hide panel"
				data-testid="hide-{id}"
				onclick={() => hide(id)}
			>
				<EyeOffIcon />
			</Button>
		</span>
	{/if}
{/snippet}

<aside class="flex flex-col" data-testid="sidebar">
	{#each panels as panel, i (panel.id)}
		{#if panel.id === 'navigation'}
			<NavigationCard {map}>
				{#snippet layoutActions()}{@render controls('navigation', i)}{/snippet}
			</NavigationCard>
		{:else if map.activeSystem}
			{#if panel.id === 'system-info'}
				<SystemInfoCard system={map.activeSystem}>
					{#snippet layoutActions()}{@render controls('system-info', i)}{/snippet}
				</SystemInfoCard>
			{:else if panel.id === 'threat'}
				<ThreatCard system={map.activeSystem}>
					{#snippet layoutActions()}{@render controls('threat', i)}{/snippet}
				</ThreatCard>
			{:else if panel.id === 'signatures'}
				<SignaturesPanel {map} system={map.activeSystem}>
					{#snippet layoutActions()}{@render controls('signatures', i)}{/snippet}
				</SignaturesPanel>
			{:else if panel.id === 'notes'}
				<NotesCard {map} system={map.activeSystem}>
					{#snippet layoutActions()}{@render controls('notes', i)}{/snippet}
				</NotesCard>
			{/if}
		{/if}
	{/each}

	{#if !map.activeSystem}
		<MapPanel testid="system-info-empty">
			<MapPanelHeader>System</MapPanelHeader>
			<MapPanelContent>
				<div class="flex flex-col items-center justify-center gap-2 p-4">
					<p class="font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase">
						Select a system
					</p>
				</div>
			</MapPanelContent>
		</MapPanel>
	{/if}

	{#if map.editingLayout && hiddenPanels.length > 0}
		<div class="flex flex-wrap items-center gap-2 border border-t-0 border-border p-2">
			<span class="font-mono text-[10px] tracking-wider text-muted-foreground uppercase">
				Hidden
			</span>
			{#each hiddenPanels as panel (panel.id)}
				<Button
					variant="outline"
					size="sm"
					class="h-7 gap-1"
					data-testid="show-{panel.id}"
					onclick={() => show(panel.id)}
				>
					<PlusIcon />
					{panel.label}
				</Button>
			{/each}
		</div>
	{/if}
</aside>
