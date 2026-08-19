<script lang="ts">
	// A system as a watcher sees it: the card from the live map with nothing that reacts.
	// Kept separate from `SystemNode` rather than adding a "read-only" prop to it, because
	// what a guest sees is a smaller thing entirely — no handles, no editor, no pilots.
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import { NODE_W, statusColor } from '$lib/map/helpers';

	let {
		node,
		nodeH,
		pos,
		sigCounts,
		connectionCount
	}: {
		node: MapSystemView;
		nodeH: number;
		pos: { x: number; y: number };
		sigCounts: { total: number; uncategorized: number; wormholes: number };
		connectionCount: number;
	} = $props();

	const ghost = $derived(node.solar_system_id === null);
	/** Holes scanned here that nothing on the map explains yet. */
	const unmapped = $derived(Math.max(0, sigCounts.wormholes - connectionCount));
</script>

<div
	class="absolute flex flex-col justify-center rounded border bg-card px-2 py-0.5 text-[11px] leading-tight shadow-sm {ghost
		? 'border-dashed bg-card/60'
		: ''}"
	data-testid="system-node"
	style:border-color={statusColor(node.status)}
	style:width="{NODE_W}px"
	style:height="{nodeH}px"
	style:left="{pos.x}px"
	style:top="{pos.y}px"
>
	<div class="flex min-w-0 items-center gap-1">
		<ClassBadge classId={node.wormhole_class_id} security={node.security_status} />
		{#if node.alias}
			<span class="shrink-0 font-medium text-foreground">{node.alias}</span>
			<span class="truncate text-muted-foreground">{node.name ?? 'Unmapped'}</span>
		{:else if node.name}
			<span class="truncate font-medium text-foreground">{node.name}</span>
		{:else}
			<span class="truncate font-medium text-muted-foreground italic">Unmapped</span>
		{/if}
		<span class="ml-auto flex shrink-0 items-center gap-1 text-muted-foreground">
			{#if sigCounts.total > 0}
				<span title="{sigCounts.total} signatures">{sigCounts.total}</span>
			{/if}
			{#if unmapped > 0}
				<span class="text-amber-500" title="{unmapped} unmapped wormholes">+{unmapped}</span>
			{/if}
		</span>
	</div>
	{#if node.occupying_group}
		<span class="truncate text-[10px] text-muted-foreground">{node.occupying_group}</span>
	{/if}
</div>
