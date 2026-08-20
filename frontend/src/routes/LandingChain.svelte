<script lang="ts">
	// The hero chain, laid out in world coordinates and routed by the map's own edge
	// router: the lines here bend exactly like the lines in the product do.
	import type { MapConnection } from '$lib/api/types/MapConnection';
	import { treeEdges } from '$lib/map/edges';
	import { NODE_W } from '$lib/map/helpers';

	const NODE_H = 46;
	const HOME = 1;

	const nodes = [
		{ id: HOME, x: 0, y: 150, badge: 'C5', badgeVar: 'c5', name: 'Home', sub: 'your staging' },
		{ id: 2, x: 340, y: 40, badge: 'C2', badgeVar: 'c2', name: 'Signatures', sub: 'pasted, synced' },
		{ id: 3, x: 340, y: 150, badge: 'LS', badgeVar: 'ls', name: 'Pilots', sub: 'live from ESI' },
		{ id: 4, x: 340, y: 260, badge: 'C4', badgeVar: 'c4', name: 'Routes', sub: 'shortest way home' }
	];

	const connections = [2, 3, 4].map(
		(to, i) => ({ id: 10 + i, from_system: HOME, to_system: to }) as MapConnection
	);

	const edges = [
		...treeEdges(
			connections,
			new Map(nodes.map((n) => [n.id, { x: n.x, y: n.y }])),
			NODE_H
		).values()
	];
</script>

<svg
	viewBox="-8 24 {340 + NODE_W + 8} 300"
	class="hidden w-full max-w-2xl sm:block"
	role="img"
	aria-label="A wormhole chain: one staging system connected to signatures, pilots and routes"
>
	{#each edges as edge (edge.id)}
		<path d={edge.d} fill="none" stroke="var(--color-border)" stroke-width="1.5" />
	{/each}
	{#each nodes as node (node.id)}
		<g>
			<rect
				x={node.x}
				y={node.y}
				width={NODE_W}
				height={NODE_H}
				rx="3"
				fill="var(--color-card)"
				stroke={node.id === HOME ? 'var(--color-ring)' : 'var(--color-border)'}
			/>
			<text x={node.x + 10} y={node.y + 20} class="text-[11px]" fill="var(--color-{node.badgeVar})">
				{node.badge}
			</text>
			<text
				x={node.x + 32}
				y={node.y + 20}
				class="fill-foreground font-mono text-[13px] font-medium"
			>
				{node.name}
			</text>
			<text x={node.x + 10} y={node.y + 36} class="fill-muted-foreground text-[11px]">
				{node.sub}
			</text>
		</g>
	{/each}
</svg>
