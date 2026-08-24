<script lang="ts">
	// The right-click menu: hand-rolled, coordinate-positioned, with CSS-hover flyouts. Which
	// menu renders follows what was clicked: empty canvas, a node, or a connection.
	import type { MapState, Menu } from '../../state/map-state.svelte';
	import ConnectionMenu from './ConnectionMenu.svelte';
	import MapMenu from './MapMenu.svelte';
	import NodeMenu from './NodeMenu.svelte';

	let { map, menu }: { map: MapState; menu: Menu } = $props();

	const connection = $derived(
		menu.target.kind === 'connection'
			? (map.connections.all.find((c) => c.id === (menu.target as { id: number }).id) ?? null)
			: null,
	);
</script>

<!-- Keep pointerdown from reaching the canvas: its background handler closes the menu,
     which would unmount these buttons before their click can fire. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed z-30 min-w-44 border border-border bg-popover py-1 shadow-md"
	data-testid="context-menu"
	style:left="{menu.x}px"
	style:top="{menu.y}px"
	onpointerdown={(ev) => ev.stopPropagation()}
	oncontextmenu={(ev) => {
		ev.preventDefault();
		ev.stopPropagation();
	}}
>
	{#if menu.target.kind === 'map'}
		<MapMenu {map} {menu} />
	{:else if menu.target.kind === 'node'}
		<NodeMenu {map} system={menu.target.system} />
	{:else if connection}
		<ConnectionMenu {map} {connection} />
	{/if}
</div>
