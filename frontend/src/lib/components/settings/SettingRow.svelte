<script lang="ts">
	// One setting: what it is on the left, the control on the right, the reason underneath.
	// The description is not decoration, a switch labelled "Suggest alias" says nothing about
	// what it will do to your map.
	import { cn } from '$lib/utils';

	let {
		label,
		description,
		disabled = false,
		blocked,
		control,
		id
	}: {
		label: string;
		description?: string;
		disabled?: boolean;
		/** Why the control is unavailable, shown in place of nothing happening. */
		blocked?: string;
		control: import('svelte').Snippet;
		id?: string;
	} = $props();
</script>

<div
	class={cn(
		'flex items-start justify-between gap-6 border-b border-border/40 py-4 last:border-b-0',
		disabled && 'opacity-60'
	)}
	data-testid="setting-row"
	data-setting={id}
>
	<div class="flex min-w-0 flex-col gap-1">
		<span class="text-sm font-medium">{label}</span>
		{#if description}
			<p class="max-w-prose text-xs leading-relaxed text-muted-foreground">{description}</p>
		{/if}
		{#if blocked}
			<p class="text-xs text-amber-500">{blocked}</p>
		{/if}
	</div>
	<div class="flex shrink-0 items-center gap-2 pt-0.5">
		{@render control()}
	</div>
</div>
