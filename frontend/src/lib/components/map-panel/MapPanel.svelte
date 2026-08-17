<script lang="ts">
	// Square-cornered panel shell (legacy MapPanel): a hairline ring rather than a border,
	// so panels sitting flush against each other share one line instead of stacking two.
	// It fills its tile, because the grid decides how big a panel is, not its content.
	import type { Snippet } from 'svelte';

	let { testid, children }: { testid?: string; children: Snippet } = $props();
</script>

<div
	class="flex h-full flex-col overflow-hidden bg-card ring-1 ring-border ring-offset-[-0.5px]"
	data-testid={testid}
>
	{@render children()}
</div>
