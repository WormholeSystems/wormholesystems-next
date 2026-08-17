<script lang="ts">
	// An ordered jump route: numbered rows rendered with the shared SystemRow (class,
	// name, region, sovereignty or effect), plus the hop marker and an optional ignore
	// button. One grid owns the tracks; rows are subgrids, so every column lines up and
	// resizes with the container.
	import XIcon from '@lucide/svelte/icons/x';

	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import SystemRow from '$lib/components/pickers/SystemRow.svelte';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';

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

<!-- Tracks: index, hop marker, then the shared system columns (class / name / region /
     holder), then the ignore slot. Rows are subgrids of these tracks. -->
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
							<!-- The signature is what you actually look for in the scanner, so show it
							     in place of the generic marker whenever the hole has one scanned. -->
							<span
								class="font-mono text-amber-500"
								data-testid="route-wormhole"
								title={step.signature
									? `Take wormhole ${step.signature}`
									: 'Take a wormhole (not scanned)'}
							>
								{step.signature ?? 'WH'}
							</span>
						{:else if step.via === 'evescout'}
							<span class="text-blue-400" title="EVE Scout connection">ES</span>
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
