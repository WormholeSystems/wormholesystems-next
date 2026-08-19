// Automatic placement: the chain laid out as a tree instead of dragged into shape.
//
// Ported from the legacy map (`map/core/layout/treeLayout.ts`, see
// docs/legacy/map-canvas.md). Positions are derived on the client and never stored: the
// map keeps whatever manual positions it has, and switching back to manual placement
// finds them untouched.

import type { MapSystemView } from '$lib/api/types/MapSystemView';

export interface TreeEdge {
	from: number;
	to: number;
}

export interface TreeInput {
	nodeIds: number[];
	edges: TreeEdge[];
	/** Pinned systems, which become the roots the branches grow out of. */
	rootIds: number[];
	/** Used as the root when nothing is pinned, i.e. the map's home system. */
	fallbackRootId?: number | null;
	/** Orders siblings, and the separate trees, down the cross axis. */
	compareNodes?: (a: number, b: number) => number;
}

export interface TreeOptions {
	/** Distance between depth levels, in world units. */
	levelGap?: number;
	/** Smallest distance between siblings, in world units. */
	siblingGap?: number;
	/** Snaps the result onto the same grid the manual map uses. */
	gridSize?: number;
	marginX?: number;
	marginY?: number;
}

/**
 * A node in the spanning forest, plus the scratch fields the layout walks mutate.
 */
interface LayoutNode {
	id: number;
	depth: number;
	parent: LayoutNode | null;
	children: LayoutNode[];
	/** 1-based position among siblings, so a shift can be spread across the subtrees between two contours. */
	siblingIndex: number;
	prelim: number;
	mod: number;
	change: number;
	shift: number;
	/** Stitches a shorter subtree's contour into its taller neighbour's. */
	thread: LayoutNode | null;
	ancestor: LayoutNode;
	/** The final cross-axis coordinate, filled in by the second walk. */
	cross: number;
}

/**
 * Lay the systems out as a left-to-right spanning forest rooted at the pinned ones.
 *
 * Every pinned system starts its own tree and everything else attaches to whichever root
 * reaches it first, so a chain reads as branches off the systems you decided matter. The
 * cross-axis packing is Reingold–Tilford in Buchheim's linear form: each subtree is laid
 * out against its siblings' contours, so a tall branch pushes the next one clear of its
 * whole extent rather than of one row.
 *
 * Returns a node top-left per system id, in world units, snapped to the grid.
 */
export function computeTreeLayout(
	input: TreeInput,
	options: TreeOptions = {}
): Map<number, { x: number; y: number }> {
	const gridSize = options.gridSize ?? 20;
	// Spacings are snapped too: a gap that is not a whole number of cells makes the
	// per-node snapping alternate between one and two cells, and the rows look ragged.
	const snap = (value: number) => Math.round(value / gridSize) * gridSize;
	// Wide enough that an edge's badge cluster, which sits at its midpoint, clears the
	// nodes on either side.
	const levelGap = snap(options.levelGap ?? 320);
	// Tight enough to read as one branch, loose enough that a row of pilots underneath a
	// node does not touch the next one.
	const siblingGap = snap(options.siblingGap ?? 60);
	const marginX = snap(options.marginX ?? 60);
	const marginY = snap(options.marginY ?? 40);

	// --- The undirected graph of systems. ---
	const adjacency = new Map<number, number[]>();
	for (const id of input.nodeIds) adjacency.set(id, []);
	for (const edge of input.edges) {
		if (edge.from === edge.to || !adjacency.has(edge.from) || !adjacency.has(edge.to)) continue;
		adjacency.get(edge.from)!.push(edge.to);
		adjacency.get(edge.to)!.push(edge.from);
	}
	for (const [id, neighbours] of adjacency) adjacency.set(id, [...new Set(neighbours)]);

	// --- Carve a spanning forest out of it, breadth-first. ---
	const depthOf = new Map<number, number>();
	const childrenOf = new Map<number, number[]>();
	for (const id of input.nodeIds) childrenOf.set(id, []);
	const visited = new Set<number>();
	const roots: number[] = [];
	const queue: number[] = [];

	const drain = () => {
		while (queue.length > 0) {
			const current = queue.shift()!;
			for (const neighbour of adjacency.get(current) ?? []) {
				if (visited.has(neighbour)) continue;
				visited.add(neighbour);
				depthOf.set(neighbour, depthOf.get(current)! + 1);
				childrenOf.get(current)!.push(neighbour);
				queue.push(neighbour);
			}
		}
	};
	const addRoot = (id: number) => {
		roots.push(id);
		depthOf.set(id, 0);
		visited.add(id);
		queue.push(id);
	};

	// Pinned first, and stranded systems after them, so the left column reads top to
	// bottom as "the systems you decided matter, then everything nothing reaches".
	const chosen = new Set<number>();
	const candidates = input.rootIds.filter((id) => adjacency.has(id));
	if (candidates.length === 0 && input.fallbackRootId != null && adjacency.has(input.fallbackRootId)) {
		candidates.push(input.fallbackRootId);
	}
	// Every root is seeded before the walk starts, so they share one front and each
	// system attaches to the root nearest it rather than to whichever went first.
	for (const root of candidates) {
		if (visited.has(root)) continue;
		chosen.add(root);
		addRoot(root);
	}
	drain();

	// Whatever the roots could not reach becomes a tree of its own, densest first, parked
	// beside the rest.
	const stranded = input.nodeIds
		.filter((id) => !visited.has(id))
		.sort((a, b) => adjacency.get(b)!.length - adjacency.get(a)!.length || a - b);
	for (const root of stranded) {
		if (visited.has(root)) continue;
		addRoot(root);
		drain();
	}

	const compare = input.compareNodes;
	if (compare) {
		for (const [, children] of childrenOf) children.sort(compare);
	}
	// The comparator orders each kind of root among itself, never across the two: a
	// stranded system with an early alias must not climb above a pinned one.
	roots.sort(
		(a, b) =>
			Number(!chosen.has(a)) - Number(!chosen.has(b)) || (compare ? compare(a, b) : 0)
	);

	// --- The forest as linked records the walks can chew on. ---
	const nodes = new Map<number, LayoutNode>();
	for (const id of input.nodeIds) {
		const node: LayoutNode = {
			id,
			depth: depthOf.get(id) ?? 0,
			parent: null,
			children: [],
			siblingIndex: 1,
			prelim: 0,
			mod: 0,
			change: 0,
			shift: 0,
			thread: null,
			ancestor: null as unknown as LayoutNode,
			cross: 0
		};
		node.ancestor = node;
		nodes.set(id, node);
	}
	for (const [id, childIds] of childrenOf) {
		const node = nodes.get(id)!;
		node.children = childIds.map((childId) => nodes.get(childId)!);
		node.children.forEach((child, index) => {
			child.parent = node;
			child.siblingIndex = index + 1;
		});
	}

	// The node continuing the left / right contour: the first / last child, or, for a
	// leaf, the thread into a neighbour's contour.
	const nextLeft = (node: LayoutNode) => node.children[0] ?? node.thread;
	const nextRight = (node: LayoutNode) => node.children[node.children.length - 1] ?? node.thread;
	const leftSiblingOf = (node: LayoutNode) =>
		node.parent && node.siblingIndex > 1 ? node.parent.children[node.siblingIndex - 2] : null;

	const moveSubtree = (left: LayoutNode, right: LayoutNode, distance: number) => {
		const subtrees = right.siblingIndex - left.siblingIndex;
		right.change -= distance / subtrees;
		right.shift += distance;
		left.change += distance / subtrees;
		right.prelim += distance;
		right.mod += distance;
	};

	// Where a shift is absorbed: the recorded ancestor when it is a sibling of `node`,
	// else the running default.
	const ancestorFor = (inner: LayoutNode, node: LayoutNode, fallback: LayoutNode) =>
		inner.ancestor.parent === node.parent ? inner.ancestor : fallback;

	const executeShifts = (node: LayoutNode) => {
		let shift = 0;
		let change = 0;
		for (let index = node.children.length - 1; index >= 0; index--) {
			const child = node.children[index];
			child.prelim += shift;
			child.mod += shift;
			change += child.change;
			shift += child.shift + change;
		}
	};

	// Slide a subtree along until its left contour clears its left siblings' right
	// contour by the sibling gap, threading the shorter side so deeper levels stay apart.
	const apportion = (node: LayoutNode, defaultAncestor: LayoutNode): LayoutNode => {
		const leftSibling = leftSiblingOf(node);
		if (!leftSibling) return defaultAncestor;

		let innerRight = node;
		let outerRight = node;
		let innerLeft = leftSibling;
		let outerLeft = node.parent!.children[0];
		let sInnerRight = innerRight.mod;
		let sOuterRight = outerRight.mod;
		let sInnerLeft = innerLeft.mod;
		let sOuterLeft = outerLeft.mod;

		while (nextRight(innerLeft) && nextLeft(innerRight)) {
			innerLeft = nextRight(innerLeft)!;
			innerRight = nextLeft(innerRight)!;
			outerLeft = nextLeft(outerLeft)!;
			outerRight = nextRight(outerRight)!;
			outerRight.ancestor = node;
			const shift = innerLeft.prelim + sInnerLeft - (innerRight.prelim + sInnerRight) + siblingGap;
			if (shift > 0) {
				moveSubtree(ancestorFor(innerLeft, node, defaultAncestor), node, shift);
				sInnerRight += shift;
				sOuterRight += shift;
			}
			sInnerLeft += innerLeft.mod;
			sInnerRight += innerRight.mod;
			sOuterLeft += outerLeft.mod;
			sOuterRight += outerRight.mod;
		}
		if (nextRight(innerLeft) && !nextRight(outerRight)) {
			outerRight.thread = nextRight(innerLeft);
			outerRight.mod += sInnerLeft - sOuterRight;
		} else if (nextLeft(innerRight) && !nextLeft(outerLeft)) {
			outerLeft.thread = nextLeft(innerRight);
			outerLeft.mod += sInnerRight - sOuterLeft;
			defaultAncestor = node;
		}
		return defaultAncestor;
	};

	// First walk: a preliminary cross position per subtree, relative to its parent,
	// resolving sibling overlaps on the way up.
	const firstWalk = (node: LayoutNode) => {
		if (node.children.length === 0) {
			const leftSibling = leftSiblingOf(node);
			node.prelim = leftSibling ? leftSibling.prelim + siblingGap : 0;
			return;
		}
		let defaultAncestor = node.children[0];
		for (const child of node.children) {
			firstWalk(child);
			defaultAncestor = apportion(child, defaultAncestor);
		}
		executeShifts(node);
		const midpoint = (node.children[0].prelim + node.children[node.children.length - 1].prelim) / 2;
		const leftSibling = leftSiblingOf(node);
		if (leftSibling) {
			node.prelim = leftSibling.prelim + siblingGap;
			node.mod = node.prelim - midpoint;
		} else {
			node.prelim = midpoint;
		}
	};

	// Second walk: sum the modifiers down each path, turning preliminary positions absolute.
	const secondWalk = (node: LayoutNode, modSum: number) => {
		node.cross = node.prelim + modSum;
		for (const child of node.children) secondWalk(child, modSum + node.mod);
	};

	// Every root hangs off one virtual super-root, so the trees are contour-packed like
	// any other siblings: a shallow tree rises into a deeper neighbour's empty rows
	// instead of being parked below its lowest node. It is never rendered.
	const superRoot: LayoutNode = {
		id: -1,
		depth: -1,
		parent: null,
		children: roots.map((id) => nodes.get(id)!),
		siblingIndex: 1,
		prelim: 0,
		mod: 0,
		change: 0,
		shift: 0,
		thread: null,
		ancestor: null as unknown as LayoutNode,
		cross: 0
	};
	superRoot.ancestor = superRoot;
	superRoot.children.forEach((child, index) => {
		child.parent = superRoot;
		child.siblingIndex = index + 1;
	});
	firstWalk(superRoot);
	secondWalk(superRoot, 0);

	// The walk centres the forest on zero, so drop the whole thing onto the top margin.
	let minCross = Infinity;
	for (const node of nodes.values()) minCross = Math.min(minCross, node.cross);
	if (!Number.isFinite(minCross)) minCross = 0;

	const positions = new Map<number, { x: number; y: number }>();
	for (const node of nodes.values()) {
		positions.set(node.id, {
			x: snap(marginX + node.depth * levelGap),
			y: snap(marginY + node.cross - minCross)
		});
	}
	return positions;
}

/**
 * The order siblings are laid out in: named systems first and alphabetically, then the
 * rest by name, so a branch reads the same way every time it is drawn.
 */
export function compareForTree(systems: Map<number, MapSystemView>) {
	return (a: number, b: number): number => {
		const left = systems.get(a);
		const right = systems.get(b);
		if (!left || !right) return a - b;
		if (left.alias && !right.alias) return -1;
		if (!left.alias && right.alias) return 1;
		const byAlias = (left.alias ?? '').localeCompare(right.alias ?? '');
		if (byAlias !== 0) return byAlias;
		return (left.name ?? '').localeCompare(right.name ?? '') || a - b;
	};
}
