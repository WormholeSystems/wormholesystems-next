<script lang="ts">
	// The signature list's column header. The widths here are the row's widths, so the two
	// have to stay together; keeping them in one component is what makes that true.
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';

	export type SortColumn = 'id' | 'category' | 'type' | 'age';

	let {
		compact = false,
		sort,
		onsort
	}: {
		compact?: boolean;
		sort?: { column: SortColumn; direction: 'asc' | 'desc' };
		/** Omitted where the header is not sortable, which leaves the arrows off. */
		onsort?: (column: SortColumn) => void;
	} = $props();
</script>

{#snippet arrow(column: SortColumn)}
	{#if sort?.column === column}
		{#if sort.direction === 'asc'}
			<ArrowUpIcon class="size-3" />
		{:else}
			<ArrowDownIcon class="size-3" />
		{/if}
	{/if}
{/snippet}

<div
	class="flex items-center gap-2 border-b border-border/30 bg-muted/20 px-3 font-mono text-[10px] tracking-wider text-muted-foreground uppercase {compact
		? 'py-0.5'
		: 'py-1.5'}"
>
	<button
		class="flex w-16 shrink-0 items-center gap-1 hover:text-foreground"
		onclick={() => onsort?.('id')}>ID {@render arrow('id')}</button
	>
	<button
		class="flex w-20 shrink-0 items-center gap-1 hover:text-foreground"
		onclick={() => onsort?.('category')}>Cat {@render arrow('category')}</button
	>
	<button
		class="flex min-w-0 flex-1 items-center gap-1 hover:text-foreground"
		onclick={() => onsort?.('type')}>Type {@render arrow('type')}</button
	>
	<span class="min-w-0 flex-1">Conn</span>
	<button
		class="flex w-10 shrink-0 items-center justify-end gap-1 hover:text-foreground"
		onclick={() => onsort?.('age')}>Age {@render arrow('age')}</button
	>
	<span class="w-12 shrink-0"></span>
</div>
