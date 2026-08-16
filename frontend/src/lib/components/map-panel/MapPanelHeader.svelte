<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children, actions }: { children: Snippet; actions?: Snippet } = $props();
</script>

<div
	class="flex h-9 shrink-0 items-center justify-between border-b border-border/50 bg-muted/30 px-3"
>
	<h3 class="flex items-center gap-1 font-mono text-[10px] tracking-wider text-muted-foreground uppercase">
		{@render children()}
	</h3>
	{#if actions}
		<div class="flex items-center gap-2">
			{@render actions()}
		</div>
	{/if}
</div>
