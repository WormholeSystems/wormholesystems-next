// The pure math under the map's pointer gestures: where a drag puts every co-dragged
// node, and what a rubber band selects.

import { NODE_W, type Vec2 } from './helpers';

/**
 * A live drag. `members` are the co-dragged nodes with their start top-left; each moves by
 * the same delta the primary moved.
 */
export interface Drag {
	primary: number;
	x: number;
	y: number;
	offX: number;
	offY: number;
	members: { id: number; sx: number; sy: number }[];
}

/** Where every member sits now: each offset by the delta the primary has moved. */
export function draggedPositions(drag: Drag): Map<number, Vec2> {
	const start = drag.members.find((m) => m.id === drag.primary);
	const dx = drag.x - (start?.sx ?? drag.x);
	const dy = drag.y - (start?.sy ?? drag.y);
	return new Map(drag.members.map((m) => [m.id, { x: m.sx + dx, y: m.sy + dy }]));
}

export interface Band {
	x0: number;
	y0: number;
	x1: number;
	y1: number;
}

/** The nodes whose center the band covers. `nodes` carry their rendered top-left. */
export function bandSelection(
	band: Band,
	nodes: { id: number; x: number; y: number }[],
	nodeH: number,
): Set<number> {
	const loX = Math.min(band.x0, band.x1);
	const hiX = Math.max(band.x0, band.x1);
	const loY = Math.min(band.y0, band.y1);
	const hiY = Math.max(band.y0, band.y1);
	const hit = nodes.filter((n) => {
		const cx = n.x + NODE_W / 2;
		const cy = n.y + nodeH / 2;
		return cx >= loX && cx <= hiX && cy >= loY && cy <= hiY;
	});
	return new Set(hit.map((n) => n.id));
}
