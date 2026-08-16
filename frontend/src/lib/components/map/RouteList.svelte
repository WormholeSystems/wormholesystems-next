<script lang="ts">
	// An ordered jump route: numbered rows rendered with the shared SystemRow
	// (class letter, name, region, sovereignty logo or effect), plus an amber WH
	// marker on wormhole hops. Resolves its own display data from the ids.
	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';

	let {
		steps
	}: {
		steps: { id: number; via?: 'stargate' | 'wormhole' | null }[];
	} = $props();

	let resolved = $state<Map<number, SystemSearchResult>>(new Map());
	$effect(() => {
		const ids = steps.map((s) => s.id).filter((id) => !resolved.has(id));
		if (ids.length === 0) return;
		api
			.resolveSystems(ids)
			.then((rows) => {
				const next = new Map(resolved);
				for (const r of rows) next.set(r.id, r);
				resolved = next;
			})
			.catch(() => {});
	});
</script>

<ol class="flex max-h-64 flex-col gap-0.5 overflow-y-auto" data-testid="route-list">
	{#each steps as step, i (i)}
		{@const r = resolved.get(step.id)}
		<li class="flex items-center gap-2 text-xs">
			{#if r}
				<SystemMenu system={r}>
					<span class="w-5 shrink-0 text-right text-muted-foreground">{i}</span>
					<span class="w-6 shrink-0 text-center">
						{#if step.via === 'wormhole'}
							<span class="text-amber-500" title="Take wormhole">WH</span>
						{/if}
					</span>
					<SystemRow system={r} />
				</SystemMenu>
			{:else}
				<span class="w-5 shrink-0 text-right text-muted-foreground">{i}</span>
				<span class="w-6 shrink-0 text-center"></span>
				<span class="text-muted-foreground">{step.id}</span>
			{/if}
		</li>
	{/each}
</ol>
