<script lang="ts">
	// A single map: the interactive graph. Systems are draggable DOM nodes on a fixed world;
	// connections are smooth curves in one SVG overlay. The world is panned (middle-mouse /
	// virtual scrollbars) and zoomed (buttons) inside a fixed-height viewport.
	//
	// Realtime: every mutation publishes a MapEvent server-side; the WS triggers a refetch.
	// Pan/zoom/selection live outside the fetched data and nodes are keyed by id, so a
	// refetch updates data in place without losing interaction state.
	import ClockIcon from '@lucide/svelte/icons/clock';
	import OrbitIcon from '@lucide/svelte/icons/orbit';
	import WeightIcon from '@lucide/svelte/icons/weight';

	import { replaceState } from '$app/navigation';
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
		nodeAt,
		sizeLetter
	} from '$lib/map/helpers';
	import type { WormholeSize } from '$lib/api/types/WormholeSize';
	import { isWormholeClass } from '$lib/map/classes';
	import { openMapSocket } from '$lib/ws';
	import ContextMenu from './ContextMenu.svelte';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import NavigationCard from './panels/NavigationCard.svelte';
	import ThreatCard from './panels/ThreatCard.svelte';
	import NotesCard from './panels/NotesCard.svelte';
	import SystemInfoCard from './panels/SystemInfoCard.svelte';
	import { MapState, type Drag } from './map-state.svelte';
	import Scrollbars from './Scrollbars.svelte';
	import SignaturesPanel from './SignaturesPanel.svelte';
	import SystemNode from './SystemNode.svelte';
	import SystemSearchDialog from './SystemSearchDialog.svelte';
	import { Switch } from '$lib/components/ui/switch';

	const mapId = $derived(Number(page.params.id) || 0);
	const map = $derived(new MapState(mapId));

	let viewportEl = $state<HTMLElement | null>(null);

	// Gestures commit only after 4px of travel (legacy hysteresis); until then they are
	// pending and a release is treated as a tap.
	const HYSTERESIS = 4;
	let pendingDrag: { cx: number; cy: number; drag: Drag } | null = null;
	let pendingBand: { cx: number; cy: number } | null = null;

	/** Legacy default ship-size heuristic for a new connection between two placements. */
	function heuristicSize(fromId: number, toId: number): WormholeSize | undefined {
		const a = map.systems.find((s) => s.id === fromId);
		const b = map.systems.find((s) => s.id === toId);
		if (!a || !b) return undefined;
		const TURNUR = 30002086;
		const classes = [a.wormhole_class_id, b.wormhole_class_id];
		if (classes.includes(13)) return 'small';
		if (classes.includes(1)) return 'medium';
		const highsec = (s: MapSystemView) => s.wormhole_class_id === 7 || s.security_status >= 0.45;
		const thera = (s: MapSystemView) => s.wormhole_class_id === 12;
		const wh = (s: MapSystemView) => isWormholeClass(s.wormhole_class_id);
		if ((thera(a) && highsec(b)) || (thera(b) && highsec(a))) return 'medium';
		if (
			(a.solar_system_id === TURNUR && wh(b)) ||
			(b.solar_system_id === TURNUR && wh(a))
		) {
			return 'medium';
		}
		return undefined;
	}

	$effect(() => {
		map.viewportEl = viewportEl;
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
		s.loadMyCharacters();
		s.fetchCharacters();
		// Presence has no realtime push yet; poll while the page is open.
		const presence = setInterval(() => s.fetchCharacters(), 15_000);
		const closeWs = openMapSocket(s.mapId, () => s.refetch());
		return () => {
			clearInterval(presence);
			closeWs();
		};
	});

	// Block the page from scrolling when the wheel is used over the canvas (we don't zoom on
	// wheel — buttons do that). Needs a non-passive listener.
	$effect(() => {
		const el = viewportEl;
		if (!el) return;
		const guard = (ev: WheelEvent) => ev.preventDefault();
		el.addEventListener('wheel', guard, { passive: false });
		return () => el.removeEventListener('wheel', guard);
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
			map.run('move', api.moveSystems({ map_id: map.mapId, moves }));
		}
		// Finish a connection drag → connect if released over a node.
		if (map.linking) {
			const l = map.linking;
			map.linking = null;
			const w = map.toWorld(ev.clientX, ev.clientY);
			const target = nodeAt(map.systems, w.x, w.y, map.grid);
			if (target !== null && target !== l.from) {
				map.run(
					'connect',
					api.addConnection({
						map_id: map.mapId,
						from_system: l.from,
						to_system: target,
						kind: 'wormhole',
						size: heuristicSize(l.from, target)
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
			pendingBand = { cx: ev.clientX, cy: ev.clientY };
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
					'remove',
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
		if (ev.button !== 0) return;
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
		const url = new URL(page.url);
		url.searchParams.set('system', String(s.solar_system_id));
		replaceState(url, {});
	}

	function handleLinkDown(ev: PointerEvent, id: number) {
		ev.stopPropagation();
		viewportEl?.setPointerCapture(ev.pointerId);
		const w = map.toWorld(ev.clientX, ev.clientY);
		map.linking = { from: id, x: w.x, y: w.y };
	}

	// --- search dialog (Add system / Add connection) ---

	function onSearchPick(solarSystemId: number) {
		const from = map.linkFrom;
		map.linkFrom = null;
		// Already placed?
		const existing = map.systems.find((s) => s.solar_system_id === solarSystemId)?.id;
		// Drop the new system at the first free grid slot near the requested spot (the
		// right-click point / source node), falling back to the viewport center.
		const base = map.searchAnchor ?? centerWorld(map.pan, map.zoom, map.viewportRect());
		map.searchAnchor = null;
		const spot = freePosition(map.systems, base, map.grid);
		map.run(
			'add',
			(async () => {
				const placement =
					existing ??
					(
						await api.addSystem({
							map_id: map.mapId,
							solar_system_id: solarSystemId,
							x: spot.x,
							y: spot.y,
							alias: null
						})
					).id;
				if (from !== null && from !== placement) {
					await api.addConnection({
						map_id: map.mapId,
						from_system: from,
						to_system: placement,
						kind: 'wormhole',
						size: heuristicSize(from, placement)
					});
				}
			})()
		);
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

	function setTracking(value: boolean) {
		api
			.updateMapUserSettings(map.mapId, { tracking_allowed: value })
			.then((s) => {
				map.userSettings = s;
				map.fetchCharacters();
			})
			.catch(() => {});
	}

	function setShowThreat(value: boolean) {
		api
			.updateMapUserSettings(map.mapId, { show_threat_level: value })
			.then((s) => (map.userSettings = s))
			.catch(() => {});
	}

	function setStaticsFirst(value: boolean) {
		api
			.updateMapUserSettings(map.mapId, { show_statics_first: value })
			.then((s) => (map.userSettings = s))
			.catch(() => {});
	}

	const connCountByPlacement = $derived.by(() => {
		const out = new Map<number, number>();
		for (const c of map.connections) {
			out.set(c.from_system, (out.get(c.from_system) ?? 0) + 1);
			out.set(c.to_system, (out.get(c.to_system) ?? 0) + 1);
		}
		return out;
	});

	function saveAlias(s: MapSystemView, alias: string | null, occupier: string | null) {
		map.run(
			'alias',
			Promise.all([
				api.setAlias({ map_id: map.mapId, map_solar_system_id: s.id, alias }),
				api.setOccupier({ map_id: map.mapId, map_solar_system_id: s.id, occupier })
			])
		);
	}

</script>

<svelte:window
	onkeydown={(ev) => {
		if (ev.key === 'Escape') map.closeMenu();
	}}
/>

<div class="flex items-center justify-between">
	<a href="/maps" class="text-sm text-muted-foreground transition-colors hover:text-foreground">
		← Maps
	</a>
	<span class="flex items-center gap-4">
		{#if map.userSettings}
			<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
				<Switch
					checked={map.userSettings.tracking_allowed}
					onCheckedChange={setTracking}
					data-testid="tracking-toggle"
				/>
				Share location
			</label>
			<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
				<Switch
					checked={map.userSettings.show_threat_level}
					onCheckedChange={setShowThreat}
					data-testid="threat-toggle"
				/>
				Threat rings
			</label>
			<label class="flex items-center gap-1.5 text-xs text-muted-foreground">
				<Switch
					checked={map.userSettings.show_statics_first}
					onCheckedChange={setStaticsFirst}
					data-testid="statics-first-toggle"
				/>
				Statics first
			</label>
		{/if}
		<span class="font-mono text-xs text-muted-foreground">{map.statusLine}</span>
	</span>
</div>

<SystemSearchDialog bind:open={map.searchOpen} onpick={onSearchPick} />

<div class="mt-3 grid grid-cols-[1fr_420px] items-start gap-4">
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_static_element_interactions -->
<div
	bind:this={viewportEl}
	data-testid="map-canvas"
	tabindex="0"
	class="group relative w-full overflow-hidden border border-border bg-zinc-950 outline-none select-none"
	style:height="{map.grid.viewport_height}px"
	onpointerdown={onBackgroundDown}
	onpointermove={onPointerMove}
	onpointerup={onPointerUp}
	onkeydown={onKey}
	oncontextmenu={(ev) => {
		ev.preventDefault();
		map.openMenu(ev.clientX, ev.clientY, { kind: 'map' });
	}}
>
	<!-- The transformed world: nodes + the connection overlay scale & pan together. -->
	<div
		class="absolute top-0 left-0 origin-top-left"
		style:width="{map.grid.world_width}px"
		style:height="{map.grid.world_height}px"
		style:background-image={gridBackground()}
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
				{@const a = map.positions.get(c.from_system)}
				{@const b = map.positions.get(c.to_system)}
				{#if a && b}
					{@const [sx, sy, ex, ey] = railAnchors(a.x, a.y, b.x, b.y, map.nodeH)}
					{@const d = curvePath(sx, sy, ex, ey)}
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
							stroke-width="4"
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-dasharray={dashed ? '2 6' : '0'}
							class="transition-opacity group-hover/edge:opacity-70"
							data-on-route={onRoute}
						/>
						<!-- Solid endpoint dots (legacy free-layout style). -->
						<circle cx={sx} cy={sy} r="4" fill={stroke} />
						<circle cx={ex} cy={ey} r="4" fill={stroke} />
						<!-- Midpoint badge cluster (legacy EdgeBadges): pill with glyph indicators. -->
						{#if badgeCount > 0}
							<foreignObject
								x={(sx + ex) / 2 - badgeWidth / 2}
								y={(sy + ey) / 2 - 10}
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
							oncontextmenu={(ev) => {
								ev.preventDefault();
								ev.stopPropagation();
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
				pos={map.positions.get(s.id) ?? { x: 0, y: 0 }}
				sigCounts={sigCountsBySystem.get(s.solar_system_id) ?? {
					total: 0,
					uncategorized: 0,
					wormholes: 0
				}}
				connectionCount={connCountByPlacement.get(s.id) ?? 0}
				pilots={pilotsBySystem.get(s.solar_system_id) ?? []}
				showThreat={map.userSettings?.show_threat_level ?? true}
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

	<!-- Zoom controls. -->
	<div class="absolute bottom-3 right-3 flex flex-col overflow-hidden border border-border bg-card">
		<button
			class="px-2.5 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
			onclick={() => map.zoomBy(1.2)}
		>
			+
		</button>
		<button
			class="border-t border-border px-2.5 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
			onclick={() => map.zoomBy(1 / 1.2)}
		>
			−
		</button>
	</div>

	{#if map.menu}
		<ContextMenu {map} menu={map.menu} />
	{/if}
</div>

<!-- Side panels for the active system (legacy layout): flush-stacked, hairline-merged. -->
<aside class="flex flex-col">
	<NavigationCard {map} />
	{#if map.activeSystem}
		<SystemInfoCard system={map.activeSystem} />
		<ThreatCard system={map.activeSystem} />
		<SignaturesPanel {map} system={map.activeSystem} />
		<NotesCard {map} system={map.activeSystem} />
	{:else}
		<MapPanel testid="system-info-empty">
			<MapPanelHeader>System</MapPanelHeader>
			<MapPanelContent>
				<div class="flex flex-col items-center justify-center gap-2 p-4">
					<p class="font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase">
						Select a system
					</p>
				</div>
			</MapPanelContent>
		</MapPanel>
	{/if}
</aside>
</div>
