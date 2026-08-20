<script lang="ts">
	// An ordered jump route: numbered rows built from the shared SystemRow, plus the hop marker
	// and an optional ignore button. One grid owns the tracks, rows are subgrids.
	import XIcon from '@lucide/svelte/icons/x';

	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import * as Tooltip from '$lib/components/ui/tooltip';

	let {
		steps,
		onignore
	}: {
		steps: {
			id: number;
			via?: 'stargate' | 'wormhole' | 'evescout' | null;
			/** For a wormhole hop, the signature to warp to on the side you are leaving. */
			signature?: string | null;
		}[];
		/** When set, middle hops get an X to route around that system. */
		onignore?: (id: number) => void;
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

<!-- Tracks: index, hop marker, the four shared system columns, then the ignore slot. -->
<Tooltip.Provider delayDuration={200}>
	<ol
		class="grid max-h-64 grid-cols-[min-content_min-content_min-content_minmax(0,1fr)_minmax(0,0.8fr)_min-content_min-content] items-center gap-x-2 overflow-y-auto"
		data-testid="route-list"
	>
		{#each steps as step, i (i)}
			{@const r = resolved.get(step.id)}
			<li class="col-span-full grid grid-cols-subgrid items-center gap-x-2 text-xs">
				{#if r}
					<SystemMenu system={r} class="col-span-full grid grid-cols-subgrid items-center gap-x-2">
						<span class="text-right text-muted-foreground">{i}</span>
						<span class="text-center">
							{#if step.via === 'wormhole'}
								<!-- The marker keeps the column narrow whatever the hop is; the signature
								     you actually punch into the scanner is one hover away. -->
								<Tooltip.Root>
									<Tooltip.Trigger class="font-mono text-amber-500" data-testid="route-wormhole">
										WH
									</Tooltip.Trigger>
									<Tooltip.Content>
										{step.signature ? `Take wormhole ${step.signature}` : 'Take a wormhole (not scanned)'}
									</Tooltip.Content>
								</Tooltip.Root>
							{:else if step.via === 'evescout'}
								<Tooltip.Root>
									<Tooltip.Trigger class="text-blue-400">ES</Tooltip.Trigger>
									<Tooltip.Content>EVE Scout connection</Tooltip.Content>
								</Tooltip.Root>
							{/if}
						</span>
						<SystemRow system={r} />
						{#if onignore}
							{#if i > 0 && i < steps.length - 1}
								<button
									class="text-muted-foreground/50 hover:text-destructive"
									title="Route around this system"
									aria-label="Ignore {r.name}"
									onclick={() => onignore(step.id)}
								>
									<XIcon class="size-3" />
								</button>
							{:else}
								<span></span>
							{/if}
						{/if}
					</SystemMenu>
				{:else}
					<span class="text-right text-muted-foreground">{i}</span>
					<span></span>
					<span class="col-span-4 truncate text-muted-foreground">{step.id}</span>
					{#if onignore}<span></span>{/if}
				{/if}
			</li>
		{/each}
	</ol>
</Tooltip.Provider>
