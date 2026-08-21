<script lang="ts">
	// Proportional scrollbars: thumb size and position are the viewport-over-world ratio at
	// the current zoom. They fade out once the view settles, since they only answer "where am
	// I in the chain" while you are navigating.
	import { clamp } from '$lib/map/helpers';
	import type { MapState } from './map-state.svelte';

	let { map }: { map: MapState } = $props();

	let hTrack: HTMLElement | null = null;
	let vTrack: HTMLElement | null = null;
	let hDragging = false;
	let vDragging = false;

	// Visible world span = viewport_size / zoom. Thumb fraction = visible / world.
	const hThumb = $derived.by(() => {
		const frac = Math.min(map.viewportRect().width / map.zoom / map.grid.world_width, 1);
		const start = clamp(-map.pan.x / map.zoom / map.grid.world_width, 0, 1 - frac);
		return { start: start * 100, size: frac * 100 };
	});
	const vThumb = $derived.by(() => {
		const frac = Math.min(map.viewportRect().height / map.zoom / map.grid.world_height, 1);
		const start = clamp(-map.pan.y / map.zoom / map.grid.world_height, 0, 1 - frac);
		return { start: start * 100, size: frac * 100 };
	});

	function hSet(clientX: number) {
		if (!hTrack) return;
		const r = hTrack.getBoundingClientRect();
		if (r.width <= 0) return;
		const cf = clamp((clientX - r.left) / r.width, 0, 1);
		const frac = hThumb.size / 100;
		const start = clamp(cf - frac / 2, 0, 1 - frac);
		map.pan = { ...map.pan, x: -start * map.zoom * map.grid.world_width };
		map.wakeScrollbars();
	}

	function vSet(clientY: number) {
		if (!vTrack) return;
		const r = vTrack.getBoundingClientRect();
		if (r.height <= 0) return;
		const cf = clamp((clientY - r.top) / r.height, 0, 1);
		const frac = vThumb.size / 100;
		const start = clamp(cf - frac / 2, 0, 1 - frac);
		map.pan = { ...map.pan, y: -start * map.zoom * map.grid.world_height };
		map.wakeScrollbars();
	}

	const thumb = 'absolute rounded-full bg-muted-foreground/50';
	// The hit area goes with the thumb: a track that still catches clicks while invisible
	// would be misleading.
	const track = $derived(map.scrollbarsVisible ? 'opacity-100' : 'pointer-events-none opacity-0');
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={hTrack}
	class="absolute inset-x-1 bottom-1 h-2 cursor-pointer transition-opacity duration-300 {track}"
	onpointerdown={(ev) => {
		ev.stopPropagation();
		hDragging = true;
		hTrack?.setPointerCapture(ev.pointerId);
		hSet(ev.clientX);
	}}
	onpointermove={(ev) => hDragging && hSet(ev.clientX)}
	onpointerup={() => (hDragging = false)}
>
	<div
		class={thumb}
		style="top:2px;bottom:2px;min-width:30px"
		style:left="{hThumb.start}%"
		style:width="{hThumb.size}%"
	></div>
</div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={vTrack}
	class="absolute inset-y-1 right-1 w-2 cursor-pointer transition-opacity duration-300 {track}"
	onpointerdown={(ev) => {
		ev.stopPropagation();
		vDragging = true;
		vTrack?.setPointerCapture(ev.pointerId);
		vSet(ev.clientY);
	}}
	onpointermove={(ev) => vDragging && vSet(ev.clientY)}
	onpointerup={() => (vDragging = false)}
>
	<div
		class={thumb}
		style="left:2px;right:2px;min-height:30px"
		style:top="{vThumb.start}%"
		style:height="{vThumb.size}%"
	></div>
</div>
