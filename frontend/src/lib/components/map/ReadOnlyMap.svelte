<script lang="ts">
	// A map somebody is watching rather than flying: the chain, and nothing that writes.
	//
	// Used for share links and public maps, where the visitor may have no account at all.
	// It shares the canvas's geometry (positions, edge routing, node card) so a shared map
	// is the same picture as the live one, and shares none of its state: no selection, no
	// panels, no socket, no settings. Refreshed on a timer, since a guest has no socket.
	import MinusIcon from '@lucide/svelte/icons/minus';
	import PlusIcon from '@lucide/svelte/icons/plus';

	import type { MapView } from '$lib/api/types/MapView';
	import type { Signature } from '$lib/api/types/Signature';
	import { freeEdges, treeEdges } from '$lib/map/edges';
	import { NODE_W, clamp, edgeColor, gridBackground, sizeLetter } from '$lib/map/helpers';
	import { compareForTree, computeTreeLayout } from '$lib/map/tree';
	import SystemNode from '$lib/components/map/ReadOnlyNode.svelte';

	let {
		view,
		signatures = [],
		cellSize = 20
	}: { view: MapView; signatures?: Signature[]; cellSize?: number } = $props();

	const nodeH = $derived(2 * cellSize);
	const systems = $derived(view.systems);
	const connections = $derived(view.connections);
	const tree = $derived(view.map.layout === 'tree');

	const positions = $derived.by(() => {
		if (!tree) {
			return new Map(systems.map((s) => [s.id, { x: s.position_x, y: s.position_y }]));
		}
		return computeTreeLayout(
			{
				nodeIds: systems.map((s) => s.id),
				edges: connections.map((c) => ({ from: c.from_system, to: c.to_system })),
				rootIds: systems.filter((s) => s.is_pinned).map((s) => s.id),
				homeId: systems.find((s) => s.is_home)?.id ?? null,
				compareNodes: compareForTree(new Map(systems.map((s) => [s.id, s])))
			},
			{ gridSize: cellSize }
		);
	});

	const geometry = $derived(
		tree ? treeEdges(connections, positions, nodeH) : freeEdges(connections, positions, nodeH)
	);

	/** How many signatures each system has, for the node's own badge. */
	const sigCounts = $derived.by(() => {
		const counts = new Map<number, { total: number; uncategorized: number; wormholes: number }>();
		for (const sig of signatures) {
			const row = counts.get(sig.solar_system_id) ?? {
				total: 0,
				uncategorized: 0,
				wormholes: 0
			};
			row.total++;
			if (sig.group === 'unknown') row.uncategorized++;
			if (sig.group === 'wormhole') row.wormholes++;
			counts.set(sig.solar_system_id, row);
		}
		return counts;
	});
	const connectionCounts = $derived.by(() => {
		const counts = new Map<number, number>();
		for (const c of connections) {
			counts.set(c.from_system, (counts.get(c.from_system) ?? 0) + 1);
			counts.set(c.to_system, (counts.get(c.to_system) ?? 0) + 1);
		}
		return counts;
	});

	// The view, which is all a watcher gets to change.
	let pan = $state({ x: 40, y: 40 });
	let zoom = $state(1);
	let dragging: { cx: number; cy: number; px: number; py: number } | null = null;

	function zoomBy(steps: number) {
		zoom = clamp(Math.round((zoom + steps * 0.1) * 10) / 10, 0.5, 2);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="relative h-full w-full overflow-hidden bg-canvas ring-1 ring-border"
	data-testid="readonly-map"
	onpointerdown={(ev) => {
		if (ev.button !== 0) return;
		(ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
		dragging = { cx: ev.clientX, cy: ev.clientY, px: pan.x, py: pan.y };
	}}
	onpointermove={(ev) => {
		if (!dragging) return;
		pan = { x: dragging.px + ev.clientX - dragging.cx, y: dragging.py + ev.clientY - dragging.cy };
	}}
	onpointerup={() => (dragging = null)}
>
	<div
		class="absolute top-0 left-0 origin-top-left"
		style:width="4000px"
		style:height="2000px"
		style:background-image={tree ? undefined : gridBackground()}
		style:background-size="{cellSize}px {cellSize}px"
		style:transform="translate({pan.x}px, {pan.y}px) scale({zoom})"
	>
		<svg class="absolute top-0 left-0 overflow-visible" width="4000" height="2000">
			{#each connections as c (c.id)}
				{@const g = geometry.get(c.id)}
				{#if g}
					{@const stroke = edgeColor(c.kind, c.mass_status, c.time_status, false)}
					{@const size = c.size !== null && c.size !== 'large' ? sizeLetter(c.size) : null}
					<path
						d={g.d}
						fill="none"
						{stroke}
						stroke-width={g.kind === 'elbow' ? 1.5 : 4}
						stroke-linecap="round"
						stroke-dasharray={c.mass_status === 'critical' || c.time_status === 'critical'
							? '2 6'
							: '0'}
					/>
					{#if g.kind === 'curve'}
						<circle cx={g.from.x} cy={g.from.y} r="4" fill={stroke} />
						<circle cx={g.to.x} cy={g.to.y} r="4" fill={stroke} />
					{/if}
					{#if size}
						<text
							x={g.center.x}
							y={g.center.y - 6}
							text-anchor="middle"
							class="fill-muted-foreground text-[11px] font-bold"
						>
							{size}
						</text>
					{/if}
				{/if}
			{/each}
		</svg>

		{#each systems as s (s.id)}
			<SystemNode
				node={s}
				{nodeH}
				pos={positions.get(s.id) ?? { x: 0, y: 0 }}
				sigCounts={sigCounts.get(s.solar_system_id ?? -1) ?? {
					total: 0,
					uncategorized: 0,
					wormholes: 0
				}}
				connectionCount={connectionCounts.get(s.id) ?? 0}
			/>
		{/each}
	</div>

	<div class="absolute right-3 bottom-3 flex items-center overflow-hidden border border-border bg-card">
		<button
			class="px-2 py-1 text-muted-foreground hover:bg-accent hover:text-foreground"
			aria-label="Zoom out"
			onpointerdown={(ev) => ev.stopPropagation()}
			onclick={() => zoomBy(-1)}
		>
			<MinusIcon class="size-3.5" />
		</button>
		<span class="border-x border-border px-2 py-1 text-xs tabular-nums text-muted-foreground">
			{Math.round(zoom * 100)}%
		</span>
		<button
			class="px-2 py-1 text-muted-foreground hover:bg-accent hover:text-foreground"
			aria-label="Zoom in"
			onpointerdown={(ev) => ev.stopPropagation()}
			onclick={() => zoomBy(1)}
		>
			<PlusIcon class="size-3.5" />
		</button>
	</div>
</div>
