<script lang="ts">
	// Wormhole Threat Analysis card for the active wormhole system: level badge, top
	// entities with kill counts and zKillboard links, analysis freshness.
	import GlobeIcon from '@lucide/svelte/icons/globe';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';

	import { api } from '$lib/api/client';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { ThreatAnalysis } from '$lib/api/types/ThreatAnalysis';
	import { Badge } from '$lib/components/ui/badge';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import EveImage from '$lib/components/EveImage.svelte';
	import { isWormholeClass } from '$lib/map/classes';

	let { system }: { system: MapSystemView } = $props();

	let analysis = $state<ThreatAnalysis | null>(null);

	$effect(() => {
		const id = system.solar_system_id;
		analysis = null;
		if (!isWormholeClass(system.wormhole_class_id)) return;
		api
			.threatAnalysis(id)
			.then((a) => (analysis = a))
			.catch(() => {});
	});

	const badgeClass = $derived.by(() => {
		switch (analysis?.threat_level) {
			case 'critical':
				return 'text-red-600 bg-red-600/10';
			case 'high':
				return 'text-orange-500 bg-orange-500/10';
			default:
				return 'text-muted-foreground bg-muted/50';
		}
	});

	function ago(iso: string): string {
		const minutes = Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60_000));
		if (minutes < 60) return `${minutes} min ago`;
		const hours = Math.round(minutes / 60);
		if (hours < 48) return `${hours} h ago`;
		return `${Math.round(hours / 24)} d ago`;
	}
</script>

{#if isWormholeClass(system.wormhole_class_id)}
	<MapPanel testid="threat-card">
		<MapPanelHeader>
			Threat Analysis
			{#snippet actions()}
				{#if analysis}
					<Badge variant="secondary" class={badgeClass} data-testid="threat-badge">
						{analysis.threat_level[0].toUpperCase() + analysis.threat_level.slice(1)}
					</Badge>
				{/if}
			{/snippet}
		</MapPanelHeader>
		<MapPanelContent>
			<div class="flex flex-col gap-2 p-3 text-xs">
			{#if !analysis}
				<p class="text-muted-foreground">No threat data available for this system.</p>
			{:else if analysis.entities.length === 0}
				<p class="text-muted-foreground">No significant activity detected.</p>
			{:else}
				<div class="flex flex-col gap-1.5">
					{#each analysis.entities as e (e.entity_type + e.id)}
						<div class="flex items-center gap-2">
							<EveImage
								kind={e.entity_type === 'alliance' ? 'alliance' : 'corporation'}
								id={e.id}
								class="size-6 rounded-sm"
							/>
							<span class="truncate">{e.name}</span>
							<span class="ml-auto shrink-0 text-muted-foreground">{e.kills} kills</span>
							<a
								href="https://zkillboard.com/{e.entity_type}/{e.id}/"
								target="_blank"
								rel="noopener"
								aria-label="zKillboard"
								class="text-muted-foreground hover:text-foreground"
							>
								<GlobeIcon class="size-3.5" />
							</a>
							<a
								href="https://zkillboard.com/{e.entity_type}/{e.id}/system/{system.solar_system_id}/"
								target="_blank"
								rel="noopener"
								aria-label="zKillboard in system"
								class="text-muted-foreground hover:text-foreground"
							>
								<MapPinIcon class="size-3.5" />
							</a>
						</div>
					{/each}
				</div>
			{/if}
			{#if analysis?.threat_analyzed_at}
				<p class="text-[10px] text-muted-foreground">
					Analyzed {ago(analysis.threat_analyzed_at)}
				</p>
			{/if}
			<a
				class="text-[10px] text-muted-foreground underline-offset-2 hover:underline"
				href="https://zkillboard.com/system/{system.solar_system_id}/"
				target="_blank"
				rel="noopener"
			>
				zKillboard
			</a>
			</div>
		</MapPanelContent>
	</MapPanel>
{/if}
