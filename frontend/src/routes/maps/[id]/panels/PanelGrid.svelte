<script lang="ts">
	// The map page laid out as a free-form grid: the canvas is a tile like any other, so
	// canvas space can be traded against panel space.
	//
	// Tiles are absolutely positioned from their {x,y,w,h} against the breakpoint's column
	// count. Placement itself lives in `$lib/layout/grid` as pure functions; this component
	// only turns pointers into calls on them.
	import { untrack } from 'svelte';

	import XIcon from '@lucide/svelte/icons/x';

	import { bottom, compact, moveItem, resizeItem, type GridItem } from '$lib/layout/grid';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import { cn } from '$lib/utils';
	import NavigationCard from './NavigationCard.svelte';
	import NotesCard from './NotesCard.svelte';
	import SystemInfoCard from './SystemInfoCard.svelte';
	import ThreatCard from './ThreatCard.svelte';
	import SignaturesPanel from '../SignaturesPanel.svelte';
	import {
		type BreakpointKey,
		type PanelId,
		breakpointFor,
		panelMeta,
		resolveLayouts
	} from './registry';
	import type { MapState } from '../map-state.svelte';
	import type { Snippet } from 'svelte';

	let { map, canvas }: { map: MapState; canvas: Snippet } = $props();

	/** How far a pointer must travel before a press becomes a drag (matches the canvas). */
	const HYSTERESIS = 4;

	let gridEl = $state<HTMLElement | null>(null);
	let gridWidth = $state(1200);
	let windowWidth = $state(1536);

	// While editing you pick a breakpoint; otherwise it follows the window.
	const activeKey = $derived<BreakpointKey>(
		map.editingLayout ? map.layoutBreakpoint : breakpointFor(windowWidth)
	);
	const layouts = $derived(resolveLayouts(map.layoutDraft));
	const layout = $derived(layouts[activeKey]);
	const hidden = $derived(new Set(map.userSettings?.hidden_panels ?? []));
	// Compacted after filtering, so hiding a panel in the middle closes the hole it leaves
	// rather than showing a gap. The stored positions are untouched, which is what lets a
	// panel go back where it was when it is unhidden.
	const items = $derived(compact(layout.items.filter((i) => !hidden.has(i.i)), layout.cols));

	const colWidth = $derived(gridWidth / layout.cols);
	const rows = $derived(bottom(items));

	/** Empty rows kept below the layout while arranging, so there is somewhere to drag a
	 *  tile *to* when it is already at the bottom. */
	const EDIT_SLACK_ROWS = 3;
	/** Rows the grid had when the current gesture started. Holding the height at that floor
	 *  stops the page shrinking under the pointer as tiles reflow, which would otherwise
	 *  jump the scroll position mid-drag. */
	let gestureFloor = $state(0);
	const gridRows = $derived(
		map.editingLayout ? Math.max(rows, gestureFloor) + EDIT_SLACK_ROWS : rows
	);

	/**
	 * A drag or resize in flight.
	 *
	 * `dx`/`dy` are the raw pixel offset: the tile being dragged follows those exactly, so it
	 * tracks the cursor rather than jumping a cell at a time. `live` is the snapped layout it
	 * would land in, which is what the other tiles reflow to and what the placeholder shows.
	 */
	let gesture = $state<{
		id: PanelId;
		kind: 'move' | 'resize';
		startX: number;
		startY: number;
		origin: GridItem;
		dx: number;
		dy: number;
		live: GridItem[] | null;
	} | null>(null);

	const shown = $derived(gesture?.live ?? items);

	/** Where the dragged tile will land, in grid units. */
	const placeholder = $derived(
		gesture?.live ? (gesture.live.find((i) => i.i === gesture!.id) ?? null) : null
	);

	/** The dragged tile's free pixel box, following the pointer. */
	const floating = $derived.by(() => {
		const g = gesture;
		if (!g?.live) return null;
		const meta = panelMeta(g.id);
		const left = g.origin.x * colWidth;
		const top = g.origin.y * layout.row_height;
		if (g.kind === 'move') {
			const width = g.origin.w * colWidth;
			// Held inside the grid rather than tracking the pointer past the edges: a tile
			// cannot land outside, and letting it hang off the right would widen the document
			// and give the whole window a horizontal scrollbar mid-drag.
			return {
				left: clamp(left + g.dx, 0, Math.max(0, gridWidth - width)),
				top: Math.max(0, top + g.dy),
				width,
				height: g.origin.h * layout.row_height
			};
		}
		// Resizing grows from the tile's own corner, and stops where the grid and the
		// panel's minimum say it must.
		return {
			left,
			top,
			width: clamp(
				g.origin.w * colWidth + g.dx,
				meta.minW * colWidth,
				(layout.cols - g.origin.x) * colWidth
			),
			height: Math.max(g.origin.h * layout.row_height + g.dy, meta.minH * layout.row_height)
		};
	});

	function clamp(v: number, lo: number, hi: number) {
		return Math.max(lo, Math.min(hi, v));
	}

	function commit(next: GridItem[]) {
		map.setLayoutItems(activeKey, next);
	}

	function onPointerDown(ev: PointerEvent, id: PanelId, kind: 'move' | 'resize') {
		if (!map.editingLayout) return;
		ev.preventDefault();
		ev.stopPropagation();
		const origin = items.find((i) => i.i === id);
		if (!origin) return;
		(ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
		gestureFloor = rows;
		gesture = { id, kind, startX: ev.clientX, startY: ev.clientY, origin, dx: 0, dy: 0, live: null };
	}

	function onPointerMove(ev: PointerEvent) {
		const g = gesture;
		if (!g) return;
		const dx = ev.clientX - g.startX;
		const dy = ev.clientY - g.startY;
		if (!g.live && Math.hypot(dx, dy) < HYSTERESIS) return;

		const meta = panelMeta(g.id);
		const cols = Math.round(dx / colWidth);
		const rowsMoved = Math.round(dy / layout.row_height);
		const next =
			g.kind === 'move'
				? moveItem(items, g.id, g.origin.x + cols, g.origin.y + rowsMoved, layout.cols)
				: resizeItem(items, g.id, g.origin.w + cols, g.origin.h + rowsMoved, layout.cols, meta);
		gesture = { ...g, dx, dy, live: next };
	}

	function onPointerUp() {
		const g = gesture;
		gesture = null;
		gestureFloor = 0;
		if (g?.live) commit(g.live);
	}

	/** Arrow keys move a focused tile, shift+arrows resize it. */
	function onKeyDown(ev: KeyboardEvent, id: PanelId) {
		if (!map.editingLayout) return;
		const deltas: Record<string, [number, number]> = {
			ArrowLeft: [-1, 0],
			ArrowRight: [1, 0],
			ArrowUp: [0, -1],
			ArrowDown: [0, 1]
		};
		const delta = deltas[ev.key];
		if (!delta) return;
		ev.preventDefault();
		const current = items.find((i) => i.i === id);
		if (!current) return;
		const [dx, dy] = delta;
		commit(
			ev.shiftKey
				? resizeItem(items, id, current.w + dx, current.h + dy, layout.cols, panelMeta(id))
				: moveItem(items, id, current.x + dx, current.y + dy, layout.cols)
		);
	}

	$effect(() => {
		const el = gridEl;
		if (!el) return;
		const observer = new ResizeObserver(([entry]) => (gridWidth = entry.contentRect.width));
		observer.observe(el);
		return () => observer.disconnect();
	});

	// Entering edit mode starts from whatever breakpoint the window is actually at, and
	// snapshots the hidden set so Discard can put it back.
	$effect(() => {
		if (map.editingLayout) {
			untrack(() => {
				map.layoutBreakpoint = breakpointFor(windowWidth);
				map.rememberHidden();
			});
		}
	});
</script>

<svelte:window bind:innerWidth={windowWidth} />

{#snippet tile(item: GridItem)}
	{@const meta = panelMeta(item.i as PanelId)}
	{@const held = gesture?.id === item.i ? floating : null}
	<div
		class={cn(
			'absolute transition-[left,top,width,height] duration-150',
			// The held tile follows the pointer directly, so it must not animate or it would
			// lag behind the cursor. Releasing re-enables the transition, which is what makes
			// it glide into the slot the placeholder was showing.
			held && 'z-30 shadow-2xl duration-0'
		)}
		style:width="{held ? held.width : item.w * colWidth}px"
		style:height="{held ? held.height : item.h * layout.row_height}px"
		style:left="{held ? held.left : item.x * colWidth}px"
		style:top="{held ? held.top : item.y * layout.row_height}px"
		data-testid="panel-tile"
		data-panel={item.i}
		data-x={item.x}
		data-y={item.y}
		data-w={item.w}
		data-h={item.h}
	>
		<div class="relative h-full">
			{#if item.i === 'map'}
				<div class="h-full">{@render canvas()}</div>
			{:else if item.i === 'navigation'}
				<NavigationCard {map} />
			{:else if item.i === 'system-info'}
				{#if map.activeSystem}
					<SystemInfoCard system={map.activeSystem} />
				{:else}
					{@render empty(item.i as PanelId, meta.label)}
				{/if}
			{:else if item.i === 'threat'}
				{#if map.activeSystem}
					<ThreatCard system={map.activeSystem} />
				{:else}
					{@render empty(item.i as PanelId, meta.label)}
				{/if}
			{:else if item.i === 'signatures'}
				{#if map.activeSystem}
					<SignaturesPanel {map} system={map.activeSystem} />
				{:else}
					{@render empty(item.i as PanelId, meta.label)}
				{/if}
			{:else if item.i === 'notes'}
				{#if map.activeSystem}
					<NotesCard {map} system={map.activeSystem} />
				{:else}
					{@render empty(item.i as PanelId, meta.label)}
				{/if}
			{/if}

			{#if map.editingLayout}
				<!-- Edit mode replaces the tile's own chrome with its own, over the whole card.
				     The shield is what makes a drag anywhere on the card move the tile instead
				     of reaching the content under it: without it, dragging across the canvas
				     would pan the map. Covering the panel header too means the panel's own
				     controls cannot be nudged while you are arranging, and the hide button has
				     somewhere to sit that does not collide with them.
				     Both are buttons rather than divs so they focus and take the arrow keys,
				     which is what makes the layout usable without a pointer. -->
				<div
					class="absolute inset-x-0 top-0 z-20 flex h-9 items-center justify-between bg-muted/90 pr-1 pl-3 backdrop-blur-sm"
				>
					<span class="font-mono text-[10px] tracking-wider text-muted-foreground uppercase">
						{meta.label}
					</span>
					{#if meta.removable}
						<button
							type="button"
							class="flex size-6 items-center justify-center rounded text-muted-foreground hover:bg-destructive hover:text-destructive-foreground"
							aria-label="Hide {meta.label}"
							data-testid="tile-hide"
							data-panel={item.i}
							onclick={() => map.hidePanel(item.i)}
						>
							<XIcon class="size-3.5" />
						</button>
					{/if}
				</div>
				<button
					type="button"
					class="absolute inset-x-0 top-9 bottom-0 z-20 cursor-move focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring"
					aria-label="Move {meta.label}. Arrow keys move, shift and arrows resize."
					data-testid="tile-shield"
					data-panel={item.i}
					onpointerdown={(ev) => onPointerDown(ev, item.i as PanelId, 'move')}
					onkeydown={(ev) => onKeyDown(ev, item.i as PanelId)}
				></button>
				<button
					type="button"
					class="group/resize absolute right-[3px] bottom-[3px] z-40 flex size-6 cursor-se-resize items-end justify-end p-1.5"
					aria-label="Resize {meta.label}"
					data-testid="tile-resize"
					data-panel={item.i}
					onpointerdown={(ev) => onPointerDown(ev, item.i as PanelId, 'resize')}
				>
					<!-- The corner grip: two strokes, as legacy drew it. The button around it is
					     bigger than the mark so it stays easy to grab. -->
					<span
						class="size-2 border-r-2 border-b-2 border-muted-foreground/70 group-hover/resize:border-foreground"
					></span>
				</button>
				<!-- The frame is a sibling drawn over the content, not an outline on it: as an
				     outline it was painted underneath the panel's own header background.
				     It is inset so it stays within its own card: tiles sit flush, so frames
				     drawn on the edges would meet at every seam and read as one heavy line
				     rather than as two cards. -->
				<div
					class="pointer-events-none absolute inset-[3px] z-40 border-2 border-dashed border-muted-foreground/60"
					data-testid="tile-frame"
				></div>
			{/if}
		</div>
	</div>
{/snippet}

{#snippet empty(id: PanelId, label: string)}
	<MapPanel testid="{id}-empty">
		<MapPanelHeader>{label}</MapPanelHeader>
		<MapPanelContent>
			<div class="flex h-full flex-col items-center justify-center gap-2 p-4">
				<p class="font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase">
					Select a system
				</p>
			</div>
		</MapPanelContent>
	</MapPanel>
{/snippet}

<!-- `overflow-x: clip` rather than `hidden`: it keeps a tile's drop shadow, or a tile
     mid-reflow, from widening the document and flashing a horizontal scrollbar, without
     making this a scroll container or affecting the vertical flow. It does not clip the
     map's context menus, which are `position: fixed` against the viewport.
     `data-dragging` is on while a gesture previews: committed positions only land once it
     clears, so that is what to wait on rather than a timeout. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={gridEl}
	class="relative w-full overflow-x-clip"
	style:height="{gridRows * layout.row_height}px"
	data-testid="panel-grid"
	data-breakpoint={activeKey}
	data-dragging={gesture !== null}
	onpointermove={onPointerMove}
	onpointerup={onPointerUp}
	onpointercancel={onPointerUp}
>
	{#if placeholder}
		<!-- Where the held tile will land. It sits under the tiles so the one being dragged
		     stays readable over it, and it does not animate: it is showing a decision about
		     which cell, so easing between cells would just lag behind that decision. -->
		<div
			class="absolute z-0 border-2 border-dashed border-muted-foreground/60 bg-muted/50"
			data-testid="tile-placeholder"
			data-panel={placeholder.i}
			data-x={placeholder.x}
			data-y={placeholder.y}
			data-w={placeholder.w}
			data-h={placeholder.h}
			style:width="{placeholder.w * colWidth}px"
			style:height="{placeholder.h * layout.row_height}px"
			style:left="{placeholder.x * colWidth}px"
			style:top="{placeholder.y * layout.row_height}px"
		></div>
	{/if}
	{#each shown as item (item.i)}
		{@render tile(item)}
	{/each}
</div>
