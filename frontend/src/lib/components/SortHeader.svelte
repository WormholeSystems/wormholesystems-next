<script lang="ts" generics="Column extends string">
	// One sortable column header: the label, and the direction arrow while it is the sorted
	// column.
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import type { Snippet } from 'svelte';

	import { cn } from '$lib/utils';

	let {
		column,
		sort,
		onsort,
		class: extra = '',
		testid,
		children,
	}: {
		column: Column;
		/** Omitted where the header is not sortable, which leaves the arrows off. */
		sort?: { column: Column; direction: 'asc' | 'desc' };
		onsort?: (column: Column) => void;
		class?: string;
		testid?: string;
		children: Snippet;
	} = $props();
</script>

<button
	class={cn('flex items-center gap-1 hover:text-foreground', extra)}
	data-testid={testid}
	onclick={() => onsort?.(column)}
>
	{@render children()}
	{#if sort?.column === column}
		{#if sort.direction === 'asc'}
			<ArrowUpIcon class="size-3" />
		{:else}
			<ArrowDownIcon class="size-3" />
		{/if}
	{/if}
</button>
