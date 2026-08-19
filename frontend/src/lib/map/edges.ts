// Edge geometry: where a connection's line starts, ends, bends, and where its badges sit.
//
// One geometry per connection, built for the whole map at once, because the tree router
// needs to see every edge to fan out the ones that share a node edge. Both layouts return
// the same shape, so the canvas draws them the same way.
//
// Everything here is in world units; the canvas transform does the scaling.

import type { MapConnection } from '$lib/api/types/MapConnection';
import { NODE_W, railEndpoint } from './helpers';

export interface Vec2 {
	x: number;
	y: number;
}

export interface EdgeGeometry {
	id: number;
	from: Vec2;
	to: Vec2;
	/** Where the badge cluster hangs. */
	center: Vec2;
	/** The SVG path. */
	d: string;
}

interface Rect {
	minX: number;
	minY: number;
	maxX: number;
	maxY: number;
	centerX: number;
	centerY: number;
}

/** Spacing between connections leaving the same node edge, in world units. */
const PARALLEL_SPACING = 14;
/** Spacing between the perpendicular runs of those connections, in world units. */
const BEND_SPACING = 16;
/** Corner radius of an elbow. */
const CORNER_RADIUS = 10;

function rectAt(position: Vec2, nodeH: number): Rect {
	return {
		minX: position.x,
		minY: position.y,
		maxX: position.x + NODE_W,
		maxY: position.y + nodeH,
		centerX: position.x + NODE_W / 2,
		centerY: position.y + nodeH / 2
	};
}

const midpoint = (from: Vec2, to: Vec2): Vec2 => ({
	x: (from.x + to.x) / 2,
	y: (from.y + to.y) / 2
});

/** The manual layout's bezier, easing horizontally between the two endpoints. */
export function curveBetween(from: Vec2, to: Vec2): string {
	const cp1x = from.x + (to.x - from.x) / 1.5;
	const cp2x = to.x - (to.x - from.x) / 1.5;
	return `M ${from.x} ${from.y} C ${cp1x} ${from.y}, ${cp2x} ${to.y}, ${to.x} ${to.y}`;
}

/**
 * Manual-layout geometry: endpoints slide along a rail through each node's centre line,
 * pulled toward the other node, joined by a curve.
 */
export function freeEdges(
	connections: MapConnection[],
	positions: ReadonlyMap<number, Vec2>,
	nodeH: number
): Map<number, EdgeGeometry> {
	const out = new Map<number, EdgeGeometry>();
	for (const c of connections) {
		const a = positions.get(c.from_system);
		const b = positions.get(c.to_system);
		if (!a || !b) continue;
		const from = railEndpoint(a.x, a.x + NODE_W, a.y + nodeH / 2, b.x + NODE_W / 2);
		const to = railEndpoint(b.x, b.x + NODE_W, b.y + nodeH / 2, a.x + NODE_W / 2);
		out.set(c.id, { id: c.id, from, to, center: midpoint(from, to), d: curveBetween(from, to) });
	}
	return out;
}

interface Routed {
	id: number;
	from: Vec2;
	to: Vec2;
	fromNormal: Vec2;
	toNormal: Vec2;
	source: Rect;
	target: Rect;
	bend: number | null;
	/** Perpendicular distance and signed offset of the far end, for ordering the fan. */
	distance: number;
	signed: number;
}

/**
 * Connects the centre of each box's facing edge, leaving perpendicular to it.
 *
 * Left and right edges are preferred whenever the boxes are separated horizontally: the
 * long run then drops through the clear lane between the two columns instead of cutting
 * down through the column of stacked siblings. Top and bottom are only used when the
 * boxes share a column, where there is no lane to use.
 */
function facingEnds(source: Rect, target: Rect) {
	const dx = target.centerX - source.centerX;
	const dy = target.centerY - source.centerY;
	const separatedX = target.minX > source.maxX || source.minX > target.maxX;
	const separatedY = target.minY > source.maxY || source.minY > target.maxY;

	if (separatedX || (!separatedY && Math.abs(dx) >= Math.abs(dy))) {
		const rightward = dx >= 0;
		return {
			from: { x: rightward ? source.maxX : source.minX, y: source.centerY },
			to: { x: rightward ? target.minX : target.maxX, y: target.centerY },
			fromNormal: { x: rightward ? 1 : -1, y: 0 },
			toNormal: { x: rightward ? -1 : 1, y: 0 }
		};
	}
	const downward = dy >= 0;
	return {
		from: { x: source.centerX, y: downward ? source.maxY : source.minY },
		to: { x: target.centerX, y: downward ? target.minY : target.maxY },
		fromNormal: { x: 0, y: downward ? 1 : -1 },
		toNormal: { x: 0, y: downward ? -1 : 1 }
	};
}

interface Port {
	endpoint: Vec2;
	normal: Vec2;
	box: Rect;
	sortKey: number;
}

/**
 * Spread the endpoints that share a node edge along it, ordered by where the other end
 * sits, so parallel lines neither overlap nor cross.
 */
function spreadSharedEdge(ports: Port[]): void {
	if (ports.length < 2) return;
	const alongY = ports[0].normal.x !== 0;
	const box = ports[0].box;
	const extent = alongY ? box.maxY - box.minY : box.maxX - box.minX;
	const spacing = Math.min(PARALLEL_SPACING, (extent * 0.7) / (ports.length - 1));
	ports.sort((a, b) => a.sortKey - b.sortKey);
	ports.forEach((port, i) => {
		const offset = (i - (ports.length - 1) / 2) * spacing;
		if (alongY) port.endpoint.y += offset;
		else port.endpoint.x += offset;
	});
}

/** The two turn points of an elbow: the perpendicular run sits at `bend`. */
function elbowCorners(edge: Routed): [Vec2, Vec2] {
	if (edge.fromNormal.x !== 0) {
		const midX = edge.bend ?? (edge.from.x + edge.to.x) / 2;
		return [
			{ x: midX, y: edge.from.y },
			{ x: midX, y: edge.to.y }
		];
	}
	const midY = edge.bend ?? (edge.from.y + edge.to.y) / 2;
	return [
		{ x: edge.from.x, y: midY },
		{ x: edge.to.x, y: midY }
	];
}

/** A polyline through `points` with every interior corner rounded. */
function roundedPath(points: Vec2[], radius: number): string {
	const pts = points.filter(
		(p, i) => i === 0 || Math.hypot(p.x - points[i - 1].x, p.y - points[i - 1].y) > 0.01
	);
	if (pts.length < 2) return '';
	let d = `M ${pts[0].x} ${pts[0].y}`;
	for (let i = 1; i < pts.length - 1; i++) {
		const prev = pts[i - 1];
		const curr = pts[i];
		const next = pts[i + 1];
		const lenIn = Math.hypot(curr.x - prev.x, curr.y - prev.y);
		const lenOut = Math.hypot(next.x - curr.x, next.y - curr.y);
		const r = Math.min(radius, lenIn / 2, lenOut / 2);
		const start = {
			x: curr.x + ((prev.x - curr.x) / lenIn) * r,
			y: curr.y + ((prev.y - curr.y) / lenIn) * r
		};
		const end = {
			x: curr.x + ((next.x - curr.x) / lenOut) * r,
			y: curr.y + ((next.y - curr.y) / lenOut) * r
		};
		d += ` L ${start.x} ${start.y} Q ${curr.x} ${curr.y} ${end.x} ${end.y}`;
	}
	const last = pts[pts.length - 1];
	return `${d} L ${last.x} ${last.y}`;
}

/**
 * Tree-layout geometry: rounded right angles between the facing edges of the two nodes.
 *
 * A global pass rather than one edge at a time, because both corrections need the whole
 * picture: endpoints sharing a node edge are fanned out along it, and the perpendicular
 * runs of the edges leaving one node are staggered so they nest instead of stacking on
 * top of each other.
 */
export function treeEdges(
	connections: MapConnection[],
	positions: ReadonlyMap<number, Vec2>,
	nodeH: number
): Map<number, EdgeGeometry> {
	const routed: Routed[] = [];
	for (const c of connections) {
		const a = positions.get(c.from_system);
		const b = positions.get(c.to_system);
		if (!a || !b) continue;
		const source = rectAt(a, nodeH);
		const target = rectAt(b, nodeH);
		const ends = facingEnds(source, target);
		routed.push({
			id: c.id,
			from: { ...ends.from },
			to: { ...ends.to },
			fromNormal: ends.fromNormal,
			toNormal: ends.toNormal,
			source,
			target,
			bend: null,
			distance: 0,
			signed: 0
		});
	}

	const shared = new Map<string, Port[]>();
	const register = (endpoint: Vec2, normal: Vec2, box: Rect, other: Rect) => {
		const key = `${box.centerX},${box.centerY}|${normal.x},${normal.y}`;
		const port: Port = {
			endpoint,
			normal,
			box,
			sortKey: normal.x !== 0 ? other.centerY : other.centerX
		};
		const ports = shared.get(key);
		if (ports) ports.push(port);
		else shared.set(key, [port]);
	};
	for (const edge of routed) {
		register(edge.from, edge.fromNormal, edge.source, edge.target);
		register(edge.to, edge.toNormal, edge.target, edge.source);
	}
	for (const ports of shared.values()) spreadSharedEdge(ports);

	// Stagger the perpendicular runs of the edges that fan out from one node. Grouped by
	// the node the fan leaves on its primary axis (the left one for horizontal links, the
	// top one for vertical), whichever end the connection was stored from, and ordered by
	// how far the far end sits so the runs do not cross.
	const fans = new Map<string, Routed[]>();
	for (const edge of routed) {
		const horizontal = edge.fromNormal.x !== 0;
		const sourceFirst = horizontal
			? edge.source.centerX <= edge.target.centerX
			: edge.source.centerY <= edge.target.centerY;
		const primary = sourceFirst ? edge.source : edge.target;
		const other = sourceFirst ? edge.target : edge.source;
		const along = horizontal ? other.centerY - primary.centerY : other.centerX - primary.centerX;
		edge.distance = Math.abs(along);
		edge.signed = along;
		const key = `${horizontal ? 'h' : 'v'}|${primary.centerX},${primary.centerY}`;
		const group = fans.get(key);
		if (group) group.push(edge);
		else fans.set(key, [edge]);
	}
	for (const group of fans.values()) {
		if (group.length < 2) continue;
		const horizontal = group[0].fromNormal.x !== 0;
		// Keep the fan inside the lane between the two columns: with more connections than
		// the spacing fits, tighten it so the outermost run still clears the neighbours.
		const gap = Math.min(
			...group.map((e) => Math.abs(horizontal ? e.to.x - e.from.x : e.to.y - e.from.y))
		);
		const spacing = Math.min(BEND_SPACING, (gap * 0.8) / (group.length - 1));
		// The farthest target bends closest to the node; ties split by side.
		group.sort((a, b) => b.distance - a.distance || a.signed - b.signed);
		group.forEach((edge, i) => {
			const base = horizontal ? (edge.from.x + edge.to.x) / 2 : (edge.from.y + edge.to.y) / 2;
			edge.bend = base + (i - (group.length - 1) / 2) * spacing;
		});
	}

	const out = new Map<number, EdgeGeometry>();
	for (const edge of routed) {
		const corners = elbowCorners(edge);
		out.set(edge.id, {
			id: edge.id,
			from: edge.from,
			to: edge.to,
			center: midpoint(corners[0], corners[1]),
			d: roundedPath([edge.from, corners[0], corners[1], edge.to], CORNER_RADIUS)
		});
	}
	return out;
}
