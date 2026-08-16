<script lang="ts">
	// Square-cornered sidebar panel shell (legacy MapPanel): hairline ring instead of a
	// border so flush-stacked panels merge into a single line.
	import type { Snippet } from 'svelte';

	let { testid, children }: { testid?: string; children: Snippet } = $props();
</script>

<div
	class="flex flex-col overflow-hidden bg-card ring-1 ring-border ring-offset-[-0.5px]"
	data-testid={testid}
>
	{@render children()}
</div>
