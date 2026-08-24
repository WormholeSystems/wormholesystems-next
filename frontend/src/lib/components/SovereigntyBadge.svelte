<script lang="ts">
	// Who holds a system, as the holder's logo with their name and ticker on hover. The
	// variant is the holder kind, which EveImage already knows how to fetch.
	import type { Sovereignty } from '$lib/api/types/Sovereignty';
	import EveImage from '$lib/components/EveImage.svelte';
	import * as Tooltip from '$lib/components/ui/tooltip';

	let {
		sovereignty,
		class: className = 'size-4 shrink-0 rounded-sm',
	}: {
		sovereignty: Sovereignty;
		class?: string;
	} = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger class="flex" data-testid="sovereignty-badge">
		<EveImage kind={sovereignty.kind} id={sovereignty.id} class={className} />
	</Tooltip.Trigger>
	<Tooltip.Content class="flex items-center gap-2">
		<EveImage kind={sovereignty.kind} id={sovereignty.id} size={64} class="size-6 rounded-sm" />
		{sovereignty.name}
		{#if 'ticker' in sovereignty}({sovereignty.ticker}){/if}
	</Tooltip.Content>
</Tooltip.Root>
