<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="flex-1 overflow-x-hidden overflow-y-auto">
	{@render children()}
</div>
