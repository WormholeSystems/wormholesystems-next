// The canvas's pointer state machine. Gestures commit only after a few pixels of travel;
// until then a release counts as a tap. What a committed gesture does lives on the map
// state; this decides which gesture a pointer sequence was.

import { api } from '$lib/api/client';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import { bandSelection, draggedPositions, type Drag } from '$lib/map/gestures';
import { heuristicSize, nodeAt } from '$lib/map/helpers';
import type { MapState } from './map-state.svelte';

/** How far a press may wander (screen px) and still count as a tap. */
const HYSTERESIS = 4;

export class MapGestures {
	private map: MapState;
	private pendingDrag: { cx: number; cy: number; drag: Drag } | null = null;
	private pendingBand: { cx: number; cy: number } | null = null;

	constructor(map: MapState) {
		this.map = map;
	}

	private updateBandSelection() {
		const map = this.map;
		const b = map.band;
		if (!b) return;
		// Rendered positions, not stored ones: an automatic layout draws nodes elsewhere.
		const nodes = map.systems.map((s) => ({
			id: s.id,
			...(map.positions.get(s.id) ?? { x: s.position_x, y: s.position_y }),
		}));
		map.selected = bandSelection(b, nodes, map.nodeH);
	}

	onPointerMove(ev: PointerEvent) {
		const map = this.map;
		const w = map.toWorld(ev.clientX, ev.clientY);
		if (this.pendingDrag) {
			if (
				Math.hypot(ev.clientX - this.pendingDrag.cx, ev.clientY - this.pendingDrag.cy) >= HYSTERESIS
			) {
				map.drag = this.pendingDrag.drag;
				this.pendingDrag = null;
			} else {
				return;
			}
		}
		if (this.pendingBand) {
			if (
				Math.hypot(ev.clientX - this.pendingBand.cx, ev.clientY - this.pendingBand.cy) >= HYSTERESIS
			) {
				map.selected = new Set();
				const start = map.toWorld(this.pendingBand.cx, this.pendingBand.cy);
				map.band = { x0: start.x, y0: start.y, x1: w.x, y1: w.y };
				this.pendingBand = null;
			} else {
				return;
			}
		}
		if (map.drag) {
			const d = map.drag;
			const nx = map.clampNodeX(map.snap(w.x - d.offX));
			const ny = map.clampNodeY(map.snap(w.y - d.offY));
			map.drag = { ...d, x: nx, y: ny };
		} else if (map.linking) {
			map.linking = { ...map.linking, x: w.x, y: w.y };
		} else if (map.band) {
			map.band = { ...map.band, x1: w.x, y1: w.y };
			// The selection follows the band live.
			this.updateBandSelection();
		} else if (map.panDrag) {
			const p = map.panDrag;
			map.pan = { x: p.px + ev.clientX - p.cx, y: p.py + ev.clientY - p.cy };
			map.wakeScrollbars();
		}
	}

	onPointerUp(ev: PointerEvent) {
		const map = this.map;
		if (map.drag) {
			const d = map.drag;
			map.drag = null;
			const moves = [...draggedPositions(d)].map(([id, p]) => ({
				map_solar_system_id: id,
				x: p.x,
				y: p.y,
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
						size: heuristicSize(map.systems, l.from, target),
					}),
				);
			}
		}
		// A tap (no band committed) clears the selection.
		if (map.band) {
			map.band = null;
		} else if (this.pendingBand) {
			map.selected = new Set();
		}
		this.pendingBand = null;
		this.pendingDrag = null;
		map.panDrag = null;
	}

	// The pointer is captured only once an interaction starts: capturing on a right-button
	// press would retarget the upcoming contextmenu event away from the node under it.
	onBackgroundDown(ev: PointerEvent) {
		const map = this.map;
		map.closeMenu();
		if (ev.button === 1) {
			ev.preventDefault();
			map.viewportEl?.setPointerCapture(ev.pointerId);
			map.panDrag = { cx: ev.clientX, cy: ev.clientY, px: map.pan.x, py: map.pan.y };
		} else if (ev.button === 0) {
			map.viewportEl?.setPointerCapture(ev.pointerId);
			// An automatic layout has nothing to drag, so a plain drag pans instead. A selection
			// modifier still belongs to the rubber band.
			const selecting = ev.shiftKey || ev.ctrlKey || ev.metaKey;
			if (map.layoutLocked && !selecting) {
				map.panDrag = { cx: ev.clientX, cy: ev.clientY, px: map.pan.x, py: map.pan.y };
			} else {
				this.pendingBand = { cx: ev.clientX, cy: ev.clientY };
			}
		}
	}

	/** Co-drags the whole (non-pinned) selection when the grabbed node is part of one. */
	onNodeDown(ev: PointerEvent, s: MapSystemView) {
		const map = this.map;
		if (ev.button !== 0 || map.layoutLocked || !map.canWrite) return;
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
		map.viewportEl?.setPointerCapture(ev.pointerId);
		// Record the grab offset so the node does not jump under the cursor.
		const w = map.toWorld(ev.clientX, ev.clientY);
		this.pendingDrag = {
			cx: ev.clientX,
			cy: ev.clientY,
			drag: {
				primary: s.id,
				x: cur.x,
				y: cur.y,
				offX: w.x - cur.x,
				offY: w.y - cur.y,
				members,
			},
		};
	}

	onLinkDown(ev: PointerEvent, id: number) {
		const map = this.map;
		ev.stopPropagation();
		if (!map.canWrite) return;
		map.viewportEl?.setPointerCapture(ev.pointerId);
		const w = map.toWorld(ev.clientX, ev.clientY);
		map.linking = { from: id, x: w.x, y: w.y };
	}
}
