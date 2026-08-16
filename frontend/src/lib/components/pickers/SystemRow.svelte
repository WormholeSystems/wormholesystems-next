<script lang="ts">
	// One solar system result row, shared by every system picker. Fixed-width columns so
	// rows align: colored class, name, region, then the holder — sovereignty for k-space,
	// the wormhole effect for J-space.
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import { classMeta, effectTextColor } from '$lib/map/classes';

	let { system }: { system: SystemSearchResult } = $props();

	const c = $derived(classMeta(system.wormhole_class_id, system.security));
	const sovLabel = $derived.by(() => {
		const sov = system.sovereignty;
		if (!sov) return null;
		return 'ticker' in sov && sov.ticker ? `[${sov.ticker}]` : sov.name;
	});
</script>

<span class="w-8 shrink-0 font-mono text-xs" style="color: var(--color-{c.token})">{c.short}</span>
<span class="min-w-0 flex-1 truncate text-foreground">{system.name}</span>
<span class="w-28 shrink-0 truncate text-right text-xs text-muted-foreground">{system.region}</span>
{#if system.effect_name}
	<span
		class="w-24 shrink-0 truncate text-right text-xs {effectTextColor(system.effect_name)}"
		title={system.effect_name}
	>
		{system.effect_name}
	</span>
{:else if sovLabel}
	<span
		class="w-24 shrink-0 truncate text-right text-xs text-muted-foreground"
		title={system.sovereignty?.name}
	>
		{sovLabel}
	</span>
{:else}
	<span class="w-24 shrink-0"></span>
{/if}
