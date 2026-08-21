<script lang="ts">
	// Says a feature is switched off on this deployment, rather than showing a control that
	// would never do anything. Self-hosters configure Discord separately from everything
	// else, and most of them do not configure it at all.
	import InfoIcon from '@lucide/svelte/icons/info';
	import type { Snippet } from 'svelte';

	let { title, children }: { title: string; children: Snippet } = $props();
</script>

<div
	class="flex items-start gap-2.5 border border-amber-500/40 bg-amber-500/5 px-3 py-2.5 text-xs"
	data-testid="instance-notice"
>
	<InfoIcon class="mt-0.5 size-3.5 shrink-0 text-amber-500" />
	<div class="flex flex-col gap-1">
		<span class="font-medium text-foreground">{title}</span>
		<span class="text-muted-foreground">{@render children()}</span>
	</div>
</div>
