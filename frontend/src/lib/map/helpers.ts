// Pure helpers for the map canvas, ported 1:1 from the old Leptos implementation.

import type { GridConfig } from '$lib/api/types/GridConfig';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { MassStatus } from '$lib/api/types/MassStatus';
import type { SystemStatus } from '$lib/api/types/SystemStatus';
import type { TimeStatus } from '$lib/api/types/TimeStatus';
import type { WormholeSize } from '$lib/api/types/WormholeSize';

/** Fixed node width (px, world space). Height is `2 * grid cell`. */
export const NODE_W = 180;

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
 * The first free, non-overlapping, grid-snapped slot at/after `base` (scanning right then
 * down in node-sized steps), so a newly added system doesn't land on top of another.
 */
export function freePosition(
	systems: MapSystemView[],
	base: { x: number; y: number },
	g: GridConfig
): { x: number; y: number } {
	const nodeH = 2 * g.cell_size;
	const snap = (v: number) => Math.round(v / g.cell_size) * g.cell_size;
	const overlaps = (x: number, y: number) =>
		systems.some((s) => Math.abs(x - s.position_x) < NODE_W && Math.abs(y - s.position_y) < nodeH);
	const bx = snap(base.x);
	const by = snap(base.y);
	const cols = Math.max(1, Math.floor(g.world_width / NODE_W));
	const rows = Math.max(1, Math.floor(g.world_height / nodeH));
	for (let r = 0; r < rows; r++) {
		for (let c = 0; c < cols; c++) {
			const x = snap(clamp(bx + c * NODE_W, 0, g.world_width - NODE_W));
			const y = snap(clamp(by + r * nodeH, 0, g.world_height - nodeH));
			if (!overlaps(x, y)) return { x, y };
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
		'linear-gradient(to right, rgba(255,255,255,0.04) 1px, transparent 1px), ' +
		'linear-gradient(to bottom, rgba(255,255,255,0.04) 1px, transparent 1px)'
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
