<script lang="ts">
	// Systems are draggable DOM nodes on a panned/zoomed world, connections one SVG overlay.
	// Every mutation publishes a MapEvent server-side and the WS triggers a refetch; pan, zoom
	// and selection live outside the fetched data and nodes are keyed by id, so a refetch
	// keeps interaction state.
	import ClockIcon from '@lucide/svelte/icons/clock';
	import OrbitIcon from '@lucide/svelte/icons/orbit';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import WeightIcon from '@lucide/svelte/icons/weight';
	import WorkflowIcon from '@lucide/svelte/icons/workflow';
	import { solarSystemId } from '$lib/map/system';

	import { setContext } from 'svelte';
	import { fade } from 'svelte/transition';

	import { afterNavigate, replaceState } from '$app/navigation';
	import { page } from '$app/state';

	import { api } from '$lib/api/client';
	import { cn } from '$lib/utils';
	import { Button } from '$lib/components/ui/button';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
	import type { MapView } from '$lib/api/types/MapView';
	import {
		NODE_W,
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
	import { curveBetween } from '$lib/map/edges';
	import { openMapSocket, openUserSocket } from '$lib/ws';
	import ConnectionPopover from './ConnectionPopover.svelte';
	import ContextMenu from './ContextMenu.svelte';
	import { MapState, type Drag } from './map-state.svelte';
	import Scrollbars from './Scrollbars.svelte';
	import SystemNode from './SystemNode.svelte';
	import CommandPalette from './CommandPalette.svelte';
	import LayoutToolbar from './panels/LayoutToolbar.svelte';
	import PanelGrid from './panels/PanelGrid.svelte';
	import CleanMapDialog from './CleanMapDialog.svelte';
	import IntroductionDialog from './IntroductionDialog.svelte';
	import StatusBar from './StatusBar.svelte';
	import TrackingDialog from './TrackingDialog.svelte';
	import { JumpTracker } from './tracking.svelte';
	import { atLeast } from '$lib/map/roles';

	const mapId = $derived(Number(page.params.id) || 0);
	let {
		data
	}: { data: { view: MapView | null; settings: MapUserSettings | null } } = $props();

	const map = $derived(new MapState(mapId, page.data.me != null, data));
	const canWrite = $derived(atLeast(map.data?.role, 'member'));
	// Rebuilt with the map, so navigating between maps never carries a half-seen jump over.
	const tracker = $derived(new JumpTracker(map));
	// The app-wide system context menu reads the map through this getter.
	setContext('map-state', () => map);

	let viewportEl = $state<HTMLElement | null>(null);

	// A cover that appears and vanishes inside a few frames reads as a flicker, so it stays
	// for a moment even when the map was quick. It is only ever a floor, never a delay: a
	// slow map keeps it until the data is in.
	// The cover starts where the map's own chrome does, leaving the app's nav above it.
	// Measured rather than assumed, since the nav can wrap.
	let chromeEl = $state<HTMLElement | null>(null);
	let coverTop = $state(0);
	$effect(() => {
		const el = chromeEl;
		if (!el) return;
		const measure = () => (coverTop = el.getBoundingClientRect().top);
		measure();
		window.addEventListener('resize', measure);
		return () => window.removeEventListener('resize', measure);
	});

	const COVER_MS = 500;
	let covered = $state(true);
	$effect(() => {
		// Re-covers when the map changes: switching maps rebuilds all of this, and the gap
		// before the new one is ready should look like loading rather than like nothing.
		void mapId;
		covered = true;
		const timer = setTimeout(() => (covered = false), COVER_MS);
		return () => clearTimeout(timer);
	});
	const revealed = $derived(map.ready && !covered);

	// Gestures commit only after 4px of travel; until then a release counts as a tap.
	const HYSTERESIS = 4;
	let pendingDrag: { cx: number; cy: number; drag: Drag } | null = null;
	let pendingBand: { cx: number; cy: number } | null = null;


	$effect(() => {
		map.viewportEl = viewportEl;
	});

	// A write, so it cannot happen in the constructor: the map state is built inside a
	// `$derived`, where mutating state is not allowed.
	$effect(() => {
		map.restoreZoom();
	});

	// `getBoundingClientRect` is not reactive, and the virtual scrollbars derive from this.
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
				s.activeId = s.systems.find((x) => solarSystemId(x) === wanted)?.id ?? null;
			}
		});
		s.loadUserSettings();
		s.loadRoutingGraph();
		s.loadIgnored();
		// Below here is about the pilot at the keyboard: presence, jump tracking, the private
		// channel. A watcher has none of it.
		if (!s.signedIn) {
			const closeShared = openMapSocket(
				s.mapId,
				(event) => {
					if (event?.type !== 'characters_changed') s.refetch();
				},
				(state) => (s.socket = state)
			);
			return () => closeShared();
		}
		tracker.refresh();
		s.fetchCharacters();
		const observe = () => tracker.refresh();
		// Movement arrives over the sockets; this is only the net under a dropped frame.
		const presence = setInterval(() => {
			s.fetchCharacters();
			observe();
		}, 120_000);
		// The character's own status change is how a jump is normally noticed within seconds.
		const closeUserWs = openUserSocket((event) => {
			if (event.type === 'character_status_changed') observe();
		});
		// Flying happens in the game client, so a jump has usually already happened by the
		// time the tab is looked at again.
		window.addEventListener('focus', observe);
		const closeWs = openMapSocket(
			s.mapId,
			(event) => {
				// Movement is its own event so a busy chain does not refetch the whole graph.
				if (event?.type === 'characters_changed') s.fetchCharacters();
				// A kill changes nothing about the graph, so only the killmail card reacts.
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
	// first paint.
	let routerReady = $state(false);
	afterNavigate(() => (routerReady = true));

	// Only ever writes the param: clearing it would race the load-time restore, which reads
	// `?system=` before the map data has arrived and an active system exists.
	$effect(() => {
		const active = map.activeSystem;
		if (!routerReady || active?.kind !== 'system') return;
		const url = new URL(page.url);
		if (url.searchParams.get('system') === String(active.solar_system_id)) return;
		url.searchParams.set('system', String(active.solar_system_id));
		replaceState(url, {});
	});

	// Wheel: plain scrolls the page, ctrl/meta is swallowed so pinch does not zoom the whole
	// app, shift pans the map. Needs a non-passive listener to be allowed to preventDefault.
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

	function updateBandSelection() {
		const b = map.band;
		if (!b) return;
		const loX = Math.min(b.x0, b.x1);
		const hiX = Math.max(b.x0, b.x1);
		const loY = Math.min(b.y0, b.y1);
		const hiY = Math.max(b.y0, b.y1);
		// Rendered positions, not stored ones: an automatic layout draws nodes elsewhere.
		const hit = map.systems
			.filter((s) => {
				const at = map.positions.get(s.id) ?? { x: s.position_x, y: s.position_y };
				const cx = at.x + NODE_W / 2;
				const cy = at.y + map.nodeH / 2;
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
			// Snap to the grid live, not just on drop.
			const d = map.drag;
			const nx = map.clampNodeX(map.snap(w.x - d.offX));
			const ny = map.clampNodeY(map.snap(w.y - d.offY));
			map.drag = { ...d, x: nx, y: ny };
		} else if (map.linking) {
			map.linking = { ...map.linking, x: w.x, y: w.y };
		} else if (map.band) {
			map.band = { ...map.band, x1: w.x, y1: w.y };
			// The selection follows the band live.
			updateBandSelection();
		} else if (map.panDrag) {
			const p = map.panDrag;
			map.pan = { x: p.px + ev.clientX - p.cx, y: p.py + ev.clientY - p.cy };
			map.wakeScrollbars();
		}
	}

	function onPointerUp(ev: PointerEvent) {
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
			// Seed the optimistic override before the refetch so nodes stay put.
			const pending = { ...map.pending };
			for (const m of moves) pending[m.map_solar_system_id] = { x: m.x, y: m.y };
			map.pending = pending;
			map.run('moveSystems', api.moveSystems({ map_id: map.mapId, moves }));
		}
		if (map.linking) {
			const l = map.linking;
			map.linking = null;
			const w = map.toWorld(ev.clientX, ev.clientY);
			const target = nodeAt(map.systems, w.x, w.y, map.grid, map.positions);
			// Dropping onto a ghost is the same claim from the other end, so it is no more
			// allowed than starting from one.
			const ghost = map.systems.some((s) => s.id === target && s.kind === 'ghost');
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

	// The pointer is captured only once an interaction starts: capturing on a right-button
	// press would retarget the upcoming contextmenu event away from the node under it.
	function onBackgroundDown(ev: PointerEvent) {
		map.closeMenu();
		if (ev.button === 1) {
			ev.preventDefault();
			viewportEl?.setPointerCapture(ev.pointerId);
			map.panDrag = { cx: ev.clientX, cy: ev.clientY, px: map.pan.x, py: map.pan.y };
		} else if (ev.button === 0) {
			viewportEl?.setPointerCapture(ev.pointerId);
			// An automatic layout has nothing to drag, so a plain drag pans instead. A selection
			// modifier still belongs to the rubber band.
			const selecting = ev.shiftKey || ev.ctrlKey || ev.metaKey;
			if (map.layoutLocked && !selecting) {
				map.panDrag = { cx: ev.clientX, cy: ev.clientY, px: map.pan.x, py: map.pan.y };
			} else {
				pendingBand = { cx: ev.clientX, cy: ev.clientY };
			}
		}
	}

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

	/** Co-drags the whole (non-pinned) selection when the grabbed node is part of one. */
	function handleNodeDown(ev: PointerEvent, s: MapSystemView) {
		if (ev.button !== 0 || map.layoutLocked || !canWrite) return;
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
		// Record the grab offset so the node does not jump under the cursor.
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
		// The active system drives the side panels and the amber ring. The marquee selection
		// is untouched.
		if (ev.button !== 0) return;
		ev.stopPropagation();
		map.activeId = s.id;
	}

	function handleLinkDown(ev: PointerEvent, id: number) {
		ev.stopPropagation();
		if (!canWrite) return;
		viewportEl?.setPointerCapture(ev.pointerId);
		const w = map.toWorld(ev.clientX, ev.clientY);
		map.linking = { from: id, x: w.x, y: w.y };
	}

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
		// A ghost holds no system yet, so only the alias is its own.
		const writes = [api.setAlias({ map_id: map.mapId, map_solar_system_id: s.id, alias })];
		if (s.kind === 'system') {
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

<div bind:this={chromeEl}>
	<StatusBar {map} />
</div>

<CommandPalette {map} bind:open={map.paletteOpen} />
<CleanMapDialog {map} bind:open={map.cleanPrompt} />
<TrackingDialog {map} {tracker} />

{#if map.loadError}
	<!-- A withdrawn link and a map that was never shared look the same to a signed-out
	     visitor, so the answer is the same too. -->
	{#if page.data.me == null}
		<div class="flex flex-col items-center gap-4 p-12 text-center" data-testid="map-error">
			<p class="text-sm text-muted-foreground">
				This map is not open to watch. The link may have been withdrawn.
			</p>
			<Button href="/login" variant="outline">Sign in</Button>
		</div>
	{:else}
		<p class="p-12 text-center text-sm text-destructive" data-testid="map-error">
			{map.loadError}
		</p>
	{/if}
{:else}
	<!-- Mounted as soon as the data is in, behind the cover: the arrangement is painted
	     while nobody is looking at it, so nothing is ever seen moving into place. -->
	{#if map.ready}
		<PanelGrid {map} {canvas} />
		{#if map.editingLayout}
			<LayoutToolbar {map} />
		{/if}
	{/if}
	{#if !revealed}
		<!-- Click-through once the map is really ready: what is left is the fade, and a
		     cover that swallows the first click of the session would be worse than no cover. -->
		<div
			class={cn(
				// Above the dialog layer: the introduction belongs to the map, so it waits for it.
				'fixed inset-x-0 bottom-0 z-60 overflow-hidden bg-card',
				'flex items-center justify-center',
				map.ready && 'pointer-events-none'
			)}
			style:top="{coverTop}px"
			data-testid="map-loading"
			out:fade={{ duration: 350 }}
		>
			<div class="flex flex-col items-center gap-5">
				<svg class="size-9 animate-spin text-muted-foreground" viewBox="0 0 36 36" fill="none">
					<circle cx="18" cy="18" r="16" stroke="currentColor" stroke-opacity="0.15" stroke-width="1.5" />
					<path
						d="M34 18A16 16 0 0 0 18 2"
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linecap="round"
					/>
				</svg>
				<div class="flex flex-col items-center gap-1.5">
					<p class="font-mono text-[10px] tracking-[0.35em] text-muted-foreground uppercase">
						Loading
					</p>
					<p class="text-sm font-medium">{map.data?.map.name ?? ''}</p>
				</div>
			</div>
		</div>
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
						<!-- The curve stops short of the node on its rail; an elbow already lands on
						     the node's edge. -->
						{#if !elbow}
							<circle cx={sx} cy={sy} r="4" fill={stroke} />
							<circle cx={ex} cy={ey} r="4" fill={stroke} />
						{/if}
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
						<!-- Wide invisible hit area, drawn last so it sits on top. -->
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
						d={curveBetween(start, { x: map.linking.x, y: map.linking.y })}
						fill="none"
						stroke="var(--color-edge)"
						stroke-width="4"
						stroke-linecap="round"
						stroke-dasharray="2 6"
					/>
				{/if}
			{/if}

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

		<!-- Keyed by id so a refetch diffs in place. -->
		{#each map.systems as s (s.id)}
			<SystemNode
				node={s}
				nodeH={map.nodeH}
				selected={map.selected.has(s.id)}
				highlighted={map.hoveredSystemId === s.id}
				pos={map.positions.get(s.id) ?? { x: 0, y: 0 }}
				sigCounts={sigCountsBySystem.get(solarSystemId(s) ?? -1) ?? {
					total: 0,
					uncategorized: 0,
					wormholes: 0
				}}
				connectionCount={connCountByPlacement.get(s.id) ?? 0}
				pilots={pilotsBySystem.get(solarSystemId(s) ?? -1) ?? []}
				showThreat={map.userSettings?.show_threat_level ?? true}
				draggable={!map.layoutLocked && canWrite}
				linkable={canWrite}
				editable={canWrite}
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

	<Scrollbars {map} />

	<!-- Picking the map's own mode clears the override, so a later change to the map still
	     reaches this viewer. -->
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

	<!-- The press is stopped here, like the scrollbars do: the canvas captures the pointer on
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

