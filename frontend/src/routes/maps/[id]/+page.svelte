<script lang="ts">
	// A single map: the interactive graph. Systems are draggable DOM nodes on a fixed world;
	// connections are smooth curves in one SVG overlay. The world is panned (middle-mouse /
	// virtual scrollbars) and zoomed (buttons) inside a fixed-height viewport.
	//
	// Realtime: every mutation publishes a MapEvent server-side; the WS triggers a refetch.
	// Pan/zoom/selection live outside the fetched data and nodes are keyed by id, so a
	// refetch updates data in place without losing interaction state.
	import ClockIcon from '@lucide/svelte/icons/clock';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import OrbitIcon from '@lucide/svelte/icons/orbit';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import WeightIcon from '@lucide/svelte/icons/weight';
	import WorkflowIcon from '@lucide/svelte/icons/workflow';

	import { setContext } from 'svelte';

	import { afterNavigate, replaceState } from '$app/navigation';
	import { page } from '$app/state';

	import { api } from '$lib/api/client';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import {
		NODE_W,
		curvePath,
		railAnchors,
		railEndpoint,
		centerWorld,
		edgeColor,
		freePosition,
		gridBackground,
		heuristicSize,
		nodeAt,
		sizeLetter
	} from '$lib/map/helpers';
	import type { WormholeSize } from '$lib/api/types/WormholeSize';
	import { isWormholeClass } from '$lib/map/classes';
	import { openMapSocket, openUserSocket } from '$lib/ws';
	import ConnectionPopover from './ConnectionPopover.svelte';
	import ContextMenu from './ContextMenu.svelte';
	import { MapState, type Drag } from './map-state.svelte';
	import Scrollbars from './Scrollbars.svelte';
	import SystemNode from './SystemNode.svelte';
	import CommandPalette from './CommandPalette.svelte';
	import LayoutToolbar from './panels/LayoutToolbar.svelte';
	import PanelGrid from './panels/PanelGrid.svelte';
	import IntroductionDialog from './IntroductionDialog.svelte';
	import StatusBar from './StatusBar.svelte';
	import TrackingDialog from './TrackingDialog.svelte';
	import { JumpTracker } from './tracking.svelte';

	const mapId = $derived(Number(page.params.id) || 0);
	const map = $derived(new MapState(mapId));
	// Rebuilt with the map, so navigating between maps never carries a half-seen jump over.
	const tracker = $derived(new JumpTracker(map));
	// Held so the status bar can bring the guide back after it has been waved away.
	// The app-wide system context menu reads the map through this getter.
	setContext('map-state', () => map);

	let viewportEl = $state<HTMLElement | null>(null);

	// Gestures commit only after 4px of travel (legacy hysteresis); until then they are
	// pending and a release is treated as a tap.
	const HYSTERESIS = 4;
	let pendingDrag: { cx: number; cy: number; drag: Drag } | null = null;
	let pendingBand: { cx: number; cy: number } | null = null;


	$effect(() => {
		map.viewportEl = viewportEl;
	});

	// Reading it back is a write, so it happens here rather than in the constructor: the
	// map state is built inside a `$derived`, where mutating state is not allowed.
	$effect(() => {
		map.restoreZoom();
	});

	// The canvas is sized by its container rather than by a fixed height, so its size has
	// to be observed: `getBoundingClientRect` is not reactive, and the virtual scrollbars
	// are derived from it.
	$effect(() => {
		const el = viewportEl;
		if (!el) return;
		const observer = new ResizeObserver(([entry]) => {
			const box = entry.contentRect;
			map.viewportSize = { width: box.width, height: box.height };
		});
		observer.observe(el);
		return () => observer.disconnect();
	});

	// Load + realtime: any frame on the map socket means "refetch".
	$effect(() => {
		const s = map;
		if (s.mapId === 0) return;
		s.loadGrid();
		const wanted = Number(page.url.searchParams.get('system'));
		s.refetch().then(() => {
			if (wanted && s.activeId === null) {
				s.activeId = s.systems.find((x) => x.solar_system_id === wanted)?.id ?? null;
			}
		});
		s.loadUserSettings();
		s.loadRoutingGraph();
		tracker.refresh();
		s.loadIgnored();
		s.fetchCharacters();
		const observe = () => tracker.refresh();
		// Presence has no realtime push yet; poll while the page is open. Own characters
		// ride along, so a missed push still gets the jump noticed within the interval.
		const presence = setInterval(() => {
			s.fetchCharacters();
			observe();
		}, 15_000);
		// The user socket fires when the character's status changes, which is how a jump is
		// normally noticed within seconds. Server-status news rides the same channel and
		// says nothing about where anyone is.
		const closeUserWs = openUserSocket((event) => {
			if (event.type === 'character_status_changed') observe();
		});
		// Coming back to the tab is the other half: flying happens in the game client, so
		// the jump has usually already happened by the time the map is looked at again.
		window.addEventListener('focus', observe);
		const closeWs = openMapSocket(
			s.mapId,
			(event) => {
				// Pilot movement is its own event so a busy chain does not refetch the whole
				// graph every five seconds just because someone is flying.
				if (event?.type === 'characters_changed') s.fetchCharacters();
				// A kill changes nothing about the graph, so only the card that shows them
				// reacts rather than the whole page refetching.
				else if (event?.type === 'killmail_received') s.killmailTick += 1;
				else s.refetch();
			},
			(state) => (s.socket = state)
		);
		return () => {
			clearInterval(presence);
			window.removeEventListener('focus', observe);
			closeUserWs();
			closeWs();
		};
	});

	// `replaceState` throws until the router has started, which is still mid-hydration on
	// first paint. afterNavigate fires once it is ready.
	let routerReady = $state(false);
	afterNavigate(() => (routerReady = true));

	// The deep link follows the active system wherever it was set from: a node click, the
	// palette, or the context menu. Keeping it in one effect means no caller can forget.
	// It only ever writes the param: clearing it would race the load-time restore, which
	// reads `?system=` before the map data has arrived and an active system exists.
	$effect(() => {
		const active = map.activeSystem;
		if (!routerReady || !active) return;
		const url = new URL(page.url);
		if (url.searchParams.get('system') === String(active.solar_system_id)) return;
		url.searchParams.set('system', String(active.solar_system_id));
		replaceState(url, {});
	});

	// Wheel over the canvas, by modifier (legacy's rules):
	//   plain      the page scrolls, so a map inside a long page still gets out of the way
	//   ctrl/meta  swallowed, so the pinch gesture does not zoom the whole app
	//   shift      pans the map, which is the only way to scroll it without a drag
	// Needs a non-passive listener to be allowed to preventDefault.
	$effect(() => {
		const el = viewportEl;
		if (!el) return;
		const onWheel = (ev: WheelEvent) => {
			if (ev.ctrlKey || ev.metaKey) {
				ev.preventDefault();
				return;
			}
			if (!ev.shiftKey) return;
			ev.preventDefault();
			map.panBy(-ev.deltaX, -ev.deltaY);
		};
		el.addEventListener('wheel', onWheel, { passive: false });
		return () => el.removeEventListener('wheel', onWheel);
	});

	// --- pointer plumbing ---

	function updateBandSelection() {
		const b = map.band;
		if (!b) return;
		const loX = Math.min(b.x0, b.x1);
		const hiX = Math.max(b.x0, b.x1);
		const loY = Math.min(b.y0, b.y1);
		const hiY = Math.max(b.y0, b.y1);
		const hit = map.systems
			.filter((s) => {
				const cx = s.position_x + NODE_W / 2;
				const cy = s.position_y + map.nodeH / 2;
				return cx >= loX && cx <= hiX && cy >= loY && cy <= hiY;
			})
			.map((s) => s.id);
		map.selected = new Set(hit);
	}

	function onPointerMove(ev: PointerEvent) {
		const w = map.toWorld(ev.clientX, ev.clientY);
		if (pendingDrag) {
			if (Math.hypot(ev.clientX - pendingDrag.cx, ev.clientY - pendingDrag.cy) >= HYSTERESIS) {
				map.drag = pendingDrag.drag;
				pendingDrag = null;
			} else {
				return;
			}
		}
		if (pendingBand) {
			if (Math.hypot(ev.clientX - pendingBand.cx, ev.clientY - pendingBand.cy) >= HYSTERESIS) {
				map.selected = new Set();
				const start = map.toWorld(pendingBand.cx, pendingBand.cy);
				map.band = { x0: start.x, y0: start.y, x1: w.x, y1: w.y };
				pendingBand = null;
			} else {
				return;
			}
		}
		if (map.drag) {
			// Snap to the grid live (not just on drop) and clamp to the world bounds.
			const d = map.drag;
			const nx = map.clampNodeX(map.snap(w.x - d.offX));
			const ny = map.clampNodeY(map.snap(w.y - d.offY));
			map.drag = { ...d, x: nx, y: ny };
		} else if (map.linking) {
			map.linking = { ...map.linking, x: w.x, y: w.y };
		} else if (map.band) {
			map.band = { ...map.band, x1: w.x, y1: w.y };
			// Legacy marquee: the selection follows the band live.
			updateBandSelection();
		} else if (map.panDrag) {
			const p = map.panDrag;
			map.pan = { x: p.px + ev.clientX - p.cx, y: p.py + ev.clientY - p.cy };
			map.wakeScrollbars();
		}
	}

	function onPointerUp(ev: PointerEvent) {
		// Finish a node drag → persist every dragged member's new position (one bulk move).
		if (map.drag) {
			const d = map.drag;
			map.drag = null;
			const start = d.members.find((m) => m.id === d.primary);
			const dx = d.x - (start?.sx ?? d.x);
			const dy = d.y - (start?.sy ?? d.y);
			const moves = d.members.map((m) => ({
				map_solar_system_id: m.id,
				x: m.sx + dx,
				y: m.sy + dy
			}));
			// Hand each new position to the optimistic override before the refetch, so nodes
			// stay put instead of flashing back.
			const pending = { ...map.pending };
			for (const m of moves) pending[m.map_solar_system_id] = { x: m.x, y: m.y };
			map.pending = pending;
			map.run('moveSystems', api.moveSystems({ map_id: map.mapId, moves }));
		}
		// Finish a connection drag → connect if released over a node.
		if (map.linking) {
			const l = map.linking;
			map.linking = null;
			const w = map.toWorld(ev.clientX, ev.clientY);
			const target = nodeAt(map.systems, w.x, w.y, map.grid);
			// Dropping onto an unmapped hole is the same claim from the other end, so it is
			// no more allowed than starting from one.
			const ghost = map.systems.some((s) => s.id === target && s.solar_system_id === null);
			if (target !== null && target !== l.from && !ghost) {
				map.run(
					'addConnection',
					api.addConnection({
						map_id: map.mapId,
						from_system: l.from,
						to_system: target,
						kind: 'wormhole',
						size: heuristicSize(map.systems, l.from, target)
					})
				);
			}
		}
		// Finish a rubber-band: the selection is already live; the band just disappears.
		// A tap (no band committed) clears the selection.
		if (map.band) {
			map.band = null;
		} else if (pendingBand) {
			map.selected = new Set();
		}
		pendingBand = null;
		pendingDrag = null;
		map.panDrag = null;
	}

	// Background press: middle = pan, left on empty = rubber-band (and clear selection/menu).
	// The pointer is captured only when an interaction starts — capturing on a right-button
	// press would retarget the upcoming contextmenu event away from the node under it.
	function onBackgroundDown(ev: PointerEvent) {
		map.closeMenu();
		if (ev.button === 1) {
			ev.preventDefault();
			viewportEl?.setPointerCapture(ev.pointerId);
			map.panDrag = { cx: ev.clientX, cy: ev.clientY, px: map.pan.x, py: map.pan.y };
		} else if (ev.button === 0) {
			viewportEl?.setPointerCapture(ev.pointerId);
			if (map.layoutLocked) {
				map.panDrag = { cx: ev.clientX, cy: ev.clientY, px: map.pan.x, py: map.pan.y };
			} else {
				pendingBand = { cx: ev.clientX, cy: ev.clientY };
			}
		}
	}

	// Delete key removes the current selection (bulk).
	function onKey(ev: KeyboardEvent) {
		if (ev.key === 'Delete' || ev.key === 'Backspace') {
			const ids = [...map.selected];
			if (ids.length > 0) {
				ev.preventDefault();
				map.selected = new Set();
				map.run(
					'removeSystems',
					api.removeSystems({ map_id: map.mapId, map_solar_system_ids: ids })
				);
			}
		}
	}

	/**
	 * Press on a node's drag handle: select it and start dragging. Co-dragged members: the
	 * whole (non-pinned) selection if the grabbed node is part of a multi-selection, else
	 * just this node. Each member's start position comes from the optimistic override, then
	 * the data.
	 */
	function handleNodeDown(ev: PointerEvent, s: MapSystemView) {
		if (ev.button !== 0 || map.layoutLocked) return;
		ev.stopPropagation();
		map.closeMenu();

		const cur = map.positions.get(s.id) ?? { x: s.position_x, y: s.position_y };
		const sel = map.selected;
		const posOf = (id: number): { x: number; y: number } | null => {
			const p = map.pending[id];
			if (p) return p;
			const sys = map.systems.find((x) => x.id === id);
			return sys ? { x: sys.position_x, y: sys.position_y } : null;
		};
		const pinned = (id: number) => map.systems.some((x) => x.id === id && x.is_pinned);
		const members =
			sel.has(s.id) && sel.size > 1
				? [...sel]
						.filter((id) => !pinned(id))
						.flatMap((id) => {
							const p = posOf(id);
							return p ? [{ id, sx: p.x, sy: p.y }] : [];
						})
				: [{ id: s.id, sx: cur.x, sy: cur.y }];

		if (!sel.has(s.id)) map.selected = new Set();
		if (s.is_pinned) return;
		viewportEl?.setPointerCapture(ev.pointerId);
		// Seed from the node's *current* position, recording the grab offset so the node
		// doesn't jump under the cursor. The drag only commits after 4px of travel.
		const w = map.toWorld(ev.clientX, ev.clientY);
		pendingDrag = {
			cx: ev.clientX,
			cy: ev.clientY,
			drag: {
				primary: s.id,
				x: cur.x,
				y: cur.y,
				offX: w.x - cur.x,
				offY: w.y - cur.y,
				members
			}
		};
	}

	function handleNodeSelect(ev: PointerEvent, s: MapSystemView) {
		// Left-click the body makes this the ACTIVE system (legacy model): it drives the
		// side panels and the amber ring. The marquee selection is untouched.
		if (ev.button !== 0) return;
		ev.stopPropagation();
		map.activeId = s.id;
	}

	function handleLinkDown(ev: PointerEvent, id: number) {
		ev.stopPropagation();
		viewportEl?.setPointerCapture(ev.pointerId);
		const w = map.toWorld(ev.clientX, ev.clientY);
		map.linking = { from: id, x: w.x, y: w.y };
	}

	// Per-system signature counts and connection counts for the node icon cluster.
	const sigCountsBySystem = $derived.by(() => {
		const out = new Map<number, { total: number; uncategorized: number; wormholes: number }>();
		for (const s of map.sigs) {
			const c = out.get(s.solar_system_id) ?? { total: 0, uncategorized: 0, wormholes: 0 };
			c.total += 1;
			if (s.group === 'unknown') c.uncategorized += 1;
			if (s.group === 'wormhole') c.wormholes += 1;
			out.set(s.solar_system_id, c);
		}
		return out;
	});
	const pilotsBySystem = $derived.by(() => {
		const out = new Map<number, typeof map.characters>();
		for (const c of map.characters) {
			if (c.solar_system_id === null) continue;
			const list = out.get(c.solar_system_id) ?? [];
			list.push(c);
			out.set(c.solar_system_id, list);
		}
		return out;
	});

	const connCountByPlacement = $derived.by(() => {
		const out = new Map<number, number>();
		for (const c of map.connections) {
			out.set(c.from_system, (out.get(c.from_system) ?? 0) + 1);
			out.set(c.to_system, (out.get(c.to_system) ?? 0) + 1);
		}
		return out;
	});

	function saveAlias(s: MapSystemView, alias: string | null, occupier: string | null) {
		// Who holds a system is intel about that system; a ghost is not one yet, and only
		// the alias is the placement's own.
		const writes = [api.setAlias({ map_id: map.mapId, map_solar_system_id: s.id, alias })];
		if (s.solar_system_id !== null) {
			writes.push(api.setOccupier({ map_id: map.mapId, map_solar_system_id: s.id, occupier }));
		}
		map.run('setAlias', Promise.all(writes));
	}

</script>

<svelte:window
	onkeydown={(ev) => {
		if (ev.key === 'Escape') {
			map.closeMenu();
			map.connectionPopover = null;
		}
		if (ev.key === 'k' && (ev.metaKey || ev.ctrlKey)) {
			ev.preventDefault();
			map.paletteOpen = !map.paletteOpen;
		}
	}}
/>

<StatusBar {map} />

<CommandPalette {map} bind:open={map.paletteOpen} />
<TrackingDialog {map} {tracker} />

{#if map.loadError}
	<p class="p-12 text-center text-sm text-destructive" data-testid="map-error">
		{map.loadError}
	</p>
{:else if !map.ready}
	<!-- Held until the arrangement is known, so tiles are never painted in the built-in
	     positions and then moved. -->
	<div
		class="flex h-96 flex-col items-center justify-center gap-3 text-muted-foreground"
		data-testid="map-loading"
	>
		<LoaderIcon class="size-5 animate-spin" />
		<p class="font-mono text-[10px] tracking-wider uppercase">Loading map</p>
	</div>
{:else}
	<PanelGrid {map} {canvas} />
	{#if map.editingLayout}
		<LayoutToolbar {map} />
	{/if}
{/if}

{#snippet canvas()}
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_static_element_interactions -->
<div
	bind:this={viewportEl}
	data-testid="map-canvas"
	tabindex="0"
	class="group relative h-full w-full overflow-hidden bg-canvas ring-1 ring-border ring-offset-[-0.5px] outline-none select-none"
	onpointerdown={onBackgroundDown}
	onpointerenter={() => map.wakeScrollbars()}
	onpointermove={onPointerMove}
	onpointerup={onPointerUp}
	onkeydown={onKey}
	oncontextmenu={(ev) => {
		ev.preventDefault();
		map.openMenu(ev.clientX, ev.clientY, { kind: 'map' });
	}}
>
	<IntroductionDialog {map} />

	<!-- The transformed world: nodes + the connection overlay scale & pan together. -->
	<div
		class="absolute top-0 left-0 origin-top-left"
		style:width="{map.grid.world_width}px"
		style:height="{map.grid.world_height}px"
		style:background-image={map.layoutLocked ? undefined : gridBackground()}
		style:background-size="{map.grid.cell_size}px {map.grid.cell_size}px"
		style:transform="translate({map.pan.x}px, {map.pan.y}px) scale({map.zoom})"
	>
		<svg
			class="absolute top-0 left-0 overflow-visible"
			style:width="{map.grid.world_width}px"
			style:height="{map.grid.world_height}px"
		>
			<!-- Edges. -->
			{#each map.connections as c (c.id)}
				{@const geometry = map.edgeGeometry.get(c.id)}
				{#if geometry}
					{@const { x: sx, y: sy } = geometry.from}
					{@const { x: ex, y: ey } = geometry.to}
					{@const d = geometry.d}
					{@const elbow = geometry.kind === 'elbow'}
					{@const onRoute = map.routeConnectionIds.has(c.id)}
					{@const stroke = edgeColor(c.kind, c.mass_status, c.time_status, onRoute)}
					{@const dashed =
						c.kind === 'wormhole' &&
						(c.mass_status === 'reduced' ||
							c.mass_status === 'critical' ||
							c.time_status === 'eol' ||
							c.time_status === 'critical')}
					{@const massColor =
						c.mass_status === 'reduced'
							? '#f59e0b'
							: c.mass_status === 'critical'
								? '#ef4444'
								: null}
					{@const timeColor =
						c.time_status === 'eol' ? '#a855f7' : c.time_status === 'critical' ? '#ef4444' : null}
					{@const sizeLabel = c.size !== null && c.size !== 'large' ? sizeLetter(c.size) : null}
					{@const badgeCount =
						(c.kind === 'stargate' ? 1 : 0) +
						(sizeLabel ? 1 : 0) +
						(massColor ? 1 : 0) +
						(timeColor ? 1 : 0)}
					{@const badgeWidth = badgeCount * 18 + 8}
					<g class="group/edge">
						<!-- Dash when the hole is degraded; fresh + healthy stays solid. -->
						<path
							{d}
							fill="none"
							{stroke}
							stroke-width={elbow ? 1.5 : 4}
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-dasharray={dashed ? '2 6' : '0'}
							class="transition-opacity group-hover/edge:opacity-70"
							data-on-route={onRoute}
						/>
						<!-- Solid endpoints, for the curve that stops short of the node on its rail.
						     An elbow already lands on the node's edge. -->
						{#if !elbow}
							<circle cx={sx} cy={sy} r="4" fill={stroke} />
							<circle cx={ex} cy={ey} r="4" fill={stroke} />
						{/if}
						<!-- Midpoint badge cluster (legacy EdgeBadges): pill with glyph indicators. -->
						{#if badgeCount > 0}
							<foreignObject
								x={geometry.center.x - badgeWidth / 2}
								y={geometry.center.y - 10}
								width={badgeWidth}
								height="20"
								class="pointer-events-none"
							>
								<div
									class="flex h-full items-center justify-center gap-0.5 rounded-full border border-neutral-300 bg-white px-1 dark:border-neutral-700 dark:bg-neutral-900"
								>
									{#if c.kind === 'stargate'}
										<OrbitIcon class="size-3.5" style="color: #0ea5e9" />
									{/if}
									{#if sizeLabel}
										<span class="text-[13px] leading-none font-bold text-neutral-500">
											{sizeLabel}
										</span>
									{/if}
									{#if massColor}
										<WeightIcon class="size-3.5" style="color: {massColor}" />
									{/if}
									{#if timeColor}
										<ClockIcon class="size-3.5" style="color: {timeColor}" />
									{/if}
								</div>
							</foreignObject>
						{/if}
						<!-- Wide invisible hit area, drawn last so it sits on top (legacy: 24px). -->
						<path
							{d}
							fill="none"
							stroke="transparent"
							stroke-width="24"
							style="cursor:pointer"
							role="presentation"
							data-testid="edge-hit"
							data-connection-id={c.id}
							onpointerdown={(ev) => ev.stopPropagation()}
							onclick={(ev) => {
								ev.stopPropagation();
								map.closeMenu();
								map.connectionPopover = { id: c.id, x: ev.clientX, y: ev.clientY };
							}}
							oncontextmenu={(ev) => {
								ev.preventDefault();
								ev.stopPropagation();
								map.connectionPopover = null;
								map.openMenu(ev.clientX, ev.clientY, { kind: 'connection', id: c.id });
							}}
						/>
					</g>
				{/if}
			{/each}

			<!-- Live connection-drag preview. -->
			{#if map.linking}
				{@const from = map.positions.get(map.linking.from)}
				{#if from}
					{@const start = railEndpoint(
						from.x,
						from.x + NODE_W,
						from.y + map.nodeH / 2,
						map.linking.x
					)}
					<path
						d={curvePath(start.x, start.y, map.linking.x, map.linking.y)}
						fill="none"
						stroke="var(--color-edge)"
						stroke-width="4"
						stroke-linecap="round"
						stroke-dasharray="2 6"
					/>
				{/if}
			{/if}

			<!-- Rubber-band rectangle. -->
			{#if map.band}
				{@const b = map.band}
				<rect
					x={Math.min(b.x0, b.x1)}
					y={Math.min(b.y0, b.y1)}
					width={Math.abs(b.x1 - b.x0)}
					height={Math.abs(b.y1 - b.y0)}
					fill="rgba(99,102,241,0.12)"
					stroke="#6366f1"
					stroke-width="1"
				/>
			{/if}
		</svg>

		<!-- Nodes (DOM, keyed by id so refetch diffs in place). -->
		{#each map.systems as s (s.id)}
			<SystemNode
				node={s}
				nodeH={map.nodeH}
				selected={map.selected.has(s.id)}
				highlighted={map.hoveredSystemId === s.id}
				pos={map.positions.get(s.id) ?? { x: 0, y: 0 }}
				sigCounts={sigCountsBySystem.get(s.solar_system_id ?? -1) ?? {
					total: 0,
					uncategorized: 0,
					wormholes: 0
				}}
				connectionCount={connCountByPlacement.get(s.id) ?? 0}
				pilots={pilotsBySystem.get(s.solar_system_id ?? -1) ?? []}
				showThreat={map.userSettings?.show_threat_level ?? true}
				draggable={!map.layoutLocked}
				signatureId={map.ghostSignatures.get(s.id) ?? null}
				onsavealias={(alias, occupier) => saveAlias(s, alias, occupier)}
				active={map.activeId === s.id}
				onselect={(ev) => handleNodeSelect(ev, s)}
				ondown={(ev) => handleNodeDown(ev, s)}
				onlink={(ev) => handleLinkDown(ev, s.id)}
				onmenu={(ev) => {
					ev.preventDefault();
					ev.stopPropagation();
					map.openMenu(ev.clientX, ev.clientY, { kind: 'node', system: s });
				}}
			/>
		{/each}
	</div>

	<!-- Virtual scrollbars (proportional thumbs reflecting viewport over the world). -->
	<Scrollbars {map} />

	<!-- Placement, when the map lets each viewer choose. Picking the map's own mode clears
	     the override, so a later change to the map still reaches you. -->
	{#if map.data?.map.allow_layout_override}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="absolute bottom-3 left-3 flex items-center overflow-hidden border border-border bg-card"
			data-testid="placement-controls"
			onpointerdown={(ev) => ev.stopPropagation()}
		>
			{#each [{ mode: 'manual', label: 'Custom placement', icon: WaypointsIcon }, { mode: 'tree', label: 'Automatic placement', icon: WorkflowIcon }] as option (option.mode)}
				{@const Icon = option.icon}
				<button
					class="px-2 py-1 {map.layout === option.mode
						? 'bg-accent text-foreground'
						: 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'}"
					aria-label={option.label}
					title={option.label}
					aria-pressed={map.layout === option.mode}
					data-testid="placement-{option.mode}"
					onclick={() => map.setLayoutOverride(option.mode as 'manual' | 'tree')}
				>
					<Icon class="size-4" />
				</button>
			{/each}
		</div>
	{/if}

	<!-- Zoom: one step per click, with the level spelled out between them.
	     The press is stopped here like the scrollbars do: the canvas captures the pointer on
	     background press, which retargets the click onto the canvas and never reaches the
	     button. -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="absolute right-3 bottom-3 flex items-center overflow-hidden border border-border bg-card"
		data-testid="zoom-controls"
		onpointerdown={(ev) => ev.stopPropagation()}
	>
		<button
			class="px-2.5 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
			aria-label="Zoom out"
			onclick={() => map.zoomBy(-1)}
		>
			−
		</button>
		<span
			class="border-x border-border px-2 py-1 text-xs tabular-nums text-muted-foreground"
			data-testid="zoom-level"
		>
			{Math.round(map.zoom * 100)}%
		</span>
		<button
			class="px-2.5 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
			aria-label="Zoom in"
			onclick={() => map.zoomBy(1)}
		>
			+
		</button>
	</div>

	{#if map.menu}
		<ContextMenu {map} menu={map.menu} />
	{/if}
	<ConnectionPopover {map} />
</div>
{/snippet}
