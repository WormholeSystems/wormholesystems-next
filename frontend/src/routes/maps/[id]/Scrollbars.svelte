<script lang="ts">
	// Custom proportional scrollbars: thumb size/position is the viewport-over-world ratio at
	// the current zoom. Click the track to jump, or drag to scroll (the thumb follows the
	// cursor). Subtle by default, brighter on hover (`group-hover` from the viewport).
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

	// Center the thumb at a client coordinate within its track, panning that axis.
	function hSet(clientX: number) {
		if (!hTrack) return;
		const r = hTrack.getBoundingClientRect();
		if (r.width <= 0) return;
		const cf = clamp((clientX - r.left) / r.width, 0, 1);
		const frac = hThumb.size / 100;
		const start = clamp(cf - frac / 2, 0, 1 - frac);
		map.pan = { ...map.pan, x: -start * map.zoom * map.grid.world_width };
	}

	function vSet(clientY: number) {
		if (!vTrack) return;
		const r = vTrack.getBoundingClientRect();
		if (r.height <= 0) return;
		const cf = clamp((clientY - r.top) / r.height, 0, 1);
		const frac = vThumb.size / 100;
		const start = clamp(cf - frac / 2, 0, 1 - frac);
		map.pan = { ...map.pan, y: -start * map.zoom * map.grid.world_height };
	}

	const thumb =
		'absolute rounded-full bg-muted-foreground/40 transition-colors group-hover:bg-muted-foreground/60';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={hTrack}
	class="absolute inset-x-1 bottom-1 h-2 cursor-pointer"
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
	class="absolute inset-y-1 right-1 w-2 cursor-pointer"
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
