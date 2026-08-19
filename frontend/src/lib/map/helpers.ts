// Pure helpers for the map canvas, ported 1:1 from the old Leptos implementation.

import type { GridConfig } from '$lib/api/types/GridConfig';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { MassStatus } from '$lib/api/types/MassStatus';
import type { SystemStatus } from '$lib/api/types/SystemStatus';
import type { TimeStatus } from '$lib/api/types/TimeStatus';
import type { WormholeSize } from '$lib/api/types/WormholeSize';
import { isWormholeClass } from '$lib/map/classes';

/** Fixed node width (px, world space). Height is `2 * grid cell`. */
export const NODE_W = 180;

/** kg → kilotons, `en-US` grouping, one decimal, no unit suffix (legacy formatKilotons). */
export function formatKt(kg: number): string {
	return (kg / 1_000_000).toLocaleString('en-US', { maximumFractionDigits: 1 });
}

/** Legacy ship-size letter by max jump mass (kg). */
export function shipSizeLetter(kg: number | null): string {
	if (kg === null || kg <= 0) return '—';
	const m = kg / 1_000_000;
	if (m >= 1000) return 'XL';
	if (m >= 62) return 'L';
	if (m >= 5) return 'M';
	return 'S';
}

/** World coords of the viewport center (where a freshly-added system lands). */
export function centerWorld(
	pan: { x: number; y: number },
	zoom: number,
	viewport: { width: number; height: number }
): { x: number; y: number } {
	return {
		x: (viewport.width / 2 - pan.x) / zoom,
		y: (viewport.height / 2 - pan.y) / zoom
	};
}

/**
 * Clear space kept between placed nodes, in grid cells.
 *
 * One cell: enough that nodes never sit flush against each other, and no more. Wider gaps
 * spread a chain of any size across the canvas, which costs more in panning than it buys
 * in legibility.
 */
export const NODE_GAP_CELLS = 1;

/**
 * The first free, grid-snapped slot at/after `base`: beside it, then down that column.
 *
 * "Free" means far enough from every placed node to leave [`NODE_GAP_CELLS`] of clear
 * space, not merely non-overlapping — so passing a node's own position returns the spot
 * beside it rather than on top of it, and callers do not each invent their own offset.
 *
 * The column is what makes a chain readable. Scanning the row first sent a system's second
 * hole one column further right, its third one further still, marching across the map and
 * through whatever else was already out there. Holes off one system are siblings, so they
 * stack under the first one instead, and only move right when the column is full.
 */
export function freePosition(
	systems: { position_x: number; position_y: number }[],
	base: { x: number; y: number },
	g: GridConfig
): { x: number; y: number } {
	const nodeH = 2 * g.cell_size;
	const gap = NODE_GAP_CELLS * g.cell_size;
	const stepX = NODE_W + gap;
	const stepY = nodeH + gap;
	const maxX = g.world_width - NODE_W;
	const maxY = g.world_height - nodeH;
	const snap = (v: number) => Math.round(v / g.cell_size) * g.cell_size;
	const crowded = (x: number, y: number) =>
		systems.some(
			(s) => Math.abs(x - s.position_x) < stepX && Math.abs(y - s.position_y) < stepY
		);
	const bx = snap(clamp(base.x, 0, maxX));
	const by = snap(clamp(base.y, 0, maxY));

	// Somewhere empty was asked for and is empty: nothing to step around.
	if (!crowded(bx, by)) return { x: bx, y: by };

	for (let column = 1; ; column++) {
		const x = snap(bx + column * stepX);
		if (x > maxX) break;
		for (let row = 0; ; row++) {
			const y = snap(by + row * stepY);
			if (y > maxY) break;
			if (!crowded(x, y)) return { x, y };
		}
	}
	return { x: bx, y: by };
}

/** The placement id whose node bounds contain the world point, if any. */
export function nodeAt(
	systems: MapSystemView[],
	wx: number,
	wy: number,
	g: GridConfig
): number | null {
	const h = 2 * g.cell_size;
	const hit = systems.find(
		(s) => wx >= s.position_x && wx <= s.position_x + NODE_W && wy >= s.position_y && wy <= s.position_y + h
	);
	return hit ? hit.id : null;
}

/** Padding of the connection endpoint "rail" inside each vertical node edge (legacy). */
export const RAIL_PADDING = 40;

/**
 * A connection endpoint that slides along a horizontal rail through the node's centre:
 * the rail runs at the vertical centre, inset RAIL_PADDING from each side, and the
 * endpoint sits at the point on it nearest the other node, so it is pulled toward the
 * far node but never closer than RAIL_PADDING to the edge.
 */
export function railEndpoint(
	minX: number,
	maxX: number,
	centerY: number,
	towardX: number
): { x: number; y: number } {
	const padding = Math.min(RAIL_PADDING, (maxX - minX) / 2);
	return { x: clamp(towardX, minX + padding, maxX - padding), y: centerY };
}

/**
 * Rail endpoints for an edge between two node top-left corners (legacy free-layout
 * routing): each end sits on its node's centre rail, pulled toward the other node's
 * centre.
 */
export function railAnchors(
	ax: number,
	ay: number,
	bx: number,
	by: number,
	nodeH: number
): [number, number, number, number] {
	const aCenterX = ax + NODE_W / 2;
	const bCenterX = bx + NODE_W / 2;
	const from = railEndpoint(ax, ax + NODE_W, ay + nodeH / 2, bCenterX);
	const to = railEndpoint(bx, bx + NODE_W, by + nodeH / 2, aCenterX);
	return [from.x, from.y, to.x, to.y];
}

/** The legacy free-layout bezier, easing horizontally between the two endpoints. */
export function curvePath(x1: number, y1: number, x2: number, y2: number): string {
	const cp1x = x1 + (x2 - x1) / 1.5;
	const cp2x = x2 - (x2 - x1) / 1.5;
	return `M ${x1} ${y1} C ${cp1x} ${y1}, ${cp2x} ${y2}, ${x2} ${y2}`;
}

export function gridBackground(): string {
	return (
		'linear-gradient(to right, var(--color-grid) 1px, transparent 1px), ' +
		'linear-gradient(to bottom, var(--color-grid) 1px, transparent 1px)'
	);
}

/** The node border / status icon color for a system's intel status. */
export function statusColor(status: SystemStatus): string {
	return `var(--color-status-${status})`;
}

/** Short ship-size label for a connection's max mass class. */
export function sizeLetter(s: WormholeSize): string {
	switch (s) {
		case 'xl':
			return 'XL';
		case 'large':
			return 'L';
		case 'medium':
			return 'M';
		case 'small':
			return 'S';
	}
}

/**
 * The connection stroke color (legacy model): on-route amber wins, stargates are sky,
 * any critical state red, reduced mass orange, EOL purple, else the neutral edge token
 * (neutral-300 light / neutral-700 dark).
 */
export function edgeColor(
	kind: 'wormhole' | 'stargate',
	mass: MassStatus | null,
	time: TimeStatus | null,
	onRoute: boolean
): string {
	if (onRoute) return '#f59e0b'; // amber-500
	if (kind === 'stargate') return '#0ea5e9'; // sky-500
	if (mass === 'critical' || time === 'critical') return '#ef4444'; // red-500
	if (mass === 'reduced') return '#f97316'; // orange-500
	if (time === 'eol') return '#a855f7'; // purple-500
	return 'var(--color-edge)';
}

export function clamp(v: number, lo: number, hi: number): number {
	return Math.min(Math.max(v, lo), hi);
}

/**
 * Legacy's default ship size for a new connection between two placements.
 *
 * A guess, not a measurement: the pair of classes rules some sizes out (a frigate hole is
 * never large), and the rest is left unset for whoever scans it to fill in.
 */
export function heuristicSize(
	systems: MapSystemView[],
	fromId: number,
	toId: number
): WormholeSize | undefined {
	const a = systems.find((s) => s.id === fromId);
	const b = systems.find((s) => s.id === toId);
	if (!a || !b) return undefined;
	const TURNUR = 30002086;
	const classes = [a.wormhole_class_id, b.wormhole_class_id];
	if (classes.includes(13)) return 'small';
	if (classes.includes(1)) return 'medium';
	const highsec = (s: MapSystemView) =>
		s.wormhole_class_id === 7 || (s.security_status ?? 0) >= 0.45;
	const thera = (s: MapSystemView) => s.wormhole_class_id === 12;
	const wh = (s: MapSystemView) => isWormholeClass(s.wormhole_class_id);
	if ((thera(a) && highsec(b)) || (thera(b) && highsec(a))) return 'medium';
	if ((a.solar_system_id === TURNUR && wh(b)) || (b.solar_system_id === TURNUR && wh(a))) {
		return 'medium';
	}
	return undefined;
}
