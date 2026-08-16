// The map page's shared state: the fetched data plus all interaction state. Interaction
// state is owned here, never derived from the data, so it survives refetches. Ported from
// the old Leptos `MapPage` signals.

import { api } from '$lib/api/client';
import type { GridConfig } from '$lib/api/types/GridConfig';
import type { CharacterRef } from '$lib/api/types/CharacterRef';
import type { MapCharacter } from '$lib/api/types/MapCharacter';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
import type { MapView } from '$lib/api/types/MapView';
import type { EveScoutEdge } from '$lib/api/types/EveScoutEdge';
import type { Signature } from '$lib/api/types/Signature';
import type { WatchlistEntry } from '$lib/api/types/WatchlistEntry';
import { NODE_W, clamp } from '$lib/map/helpers';

/**
 * A live drag. `primary` is the grabbed node; `x`/`y` is its current (snapped) top-left and
 * `offX`/`offY` the grab point relative to it. `members` are all co-dragged nodes (the
 * primary plus the rest of a multi-selection), each with its start top-left — every member
 * moves by the same delta the primary moved.
 */
export interface Drag {
	primary: number;
	x: number;
	y: number;
	offX: number;
	offY: number;
	members: { id: number; sx: number; sy: number }[];
}

/** An in-progress connection drag: from this placement to the current cursor (world coords). */
export interface Linking {
	from: number;
	x: number;
	y: number;
}

export type MenuTarget =
	| { kind: 'map' }
	| { kind: 'node'; system: MapSystemView }
	| { kind: 'connection'; id: number };

/** An open right-click menu, positioned at screen `(x, y)`. */
export interface Menu {
	x: number;
	y: number;
	target: MenuTarget;
}

const defaultGrid: GridConfig = {
	cell_size: 20,
	world_width: 4000,
	world_height: 2000,
	viewport_height: 1400
};

export class MapState {
	mapId: number;
	viewportEl: HTMLElement | null = null;

	data = $state<MapView | null>(null);
	grid = $state<GridConfig>(defaultGrid);
	sigs = $state<Signature[]>([]);
	watchlist = $state<WatchlistEntry[]>([]);
	eveScout = $state<EveScoutEdge[]>([]);
	characters = $state<MapCharacter[]>([]);
	myCharacters = $state<CharacterRef[]>([]);
	userSettings = $state<MapUserSettings | null>(null);
	statusLine = $state('');

	pan = $state({ x: 0, y: 0 });
	zoom = $state(1);
	selected = $state<Set<number>>(new Set());
	drag = $state<Drag | null>(null);
	// Optimistic positions held from drop until the server confirms them, so a moved node
	// doesn't flash back to its old spot during the refetch round-trip.
	pending = $state<Record<number, { x: number; y: number }>>({});
	linking = $state<Linking | null>(null);
	band = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
	menu = $state<Menu | null>(null);
	panDrag = $state<{ cx: number; cy: number; px: number; py: number } | null>(null);
	searchOpen = $state(false);
	// The connection details popover: which edge, anchored at which screen point.
	connectionPopover = $state<{ id: number; x: number; y: number } | null>(null);
	// Where a search-added system should land (world coords, top-left). Set by the
	// context menu (right-click spot / next to the source node); null = viewport center.
	searchAnchor = $state<{ x: number; y: number } | null>(null);
	// The active system (legacy model): set by clicking a node body, drives the side
	// panels and the amber ring. Independent of the marquee selection.
	activeId = $state<number | null>(null);
	// When set, a search pick also connects the new placement to this node.
	linkFrom = $state<number | null>(null);
	// Route planner: origin/destination (solar system ids) and the computed path, set by
	// the navigation card. The path drives the edge highlight.
	routeFromId = $state<number | null>(null);
	routeToId = $state<number | null>(null);
	routePath = $state<number[]>([]);
	// Systems the router steers around (per map, persisted locally).
	ignoredSystems = $state<Set<number>>(new Set());

	systems = $derived(this.data?.systems ?? []);
	activeSystem = $derived(this.systems.find((s) => s.id === this.activeId) ?? null);
	/** One origin for watchlist/find distances: route From, else the active system,
	 *  else the tracked character's location. */
	routeOrigin = $derived(
		this.routeFromId ??
			this.activeSystem?.solar_system_id ??
			this.myCharacters.find((c) => c.is_active && c.online)?.solar_system_id ??
			this.myCharacters.find((c) => c.online && c.solar_system_id !== null)?.solar_system_id ??
			null
	);

	private ignoreStorageKey(): string {
		return `route-ignored-${this.mapId}`;
	}

	loadIgnored() {
		try {
			const raw = localStorage.getItem(this.ignoreStorageKey());
			this.ignoredSystems = new Set(raw ? (JSON.parse(raw) as number[]) : []);
		} catch {
			this.ignoredSystems = new Set();
		}
	}

	ignoreSystem(id: number) {
		const next = new Set(this.ignoredSystems);
		next.add(id);
		this.ignoredSystems = next;
		localStorage.setItem(this.ignoreStorageKey(), JSON.stringify([...next]));
	}

	clearIgnored() {
		this.ignoredSystems = new Set();
		localStorage.removeItem(this.ignoreStorageKey());
	}

	async loadEveScout() {
		try {
			this.eveScout = await api.eveScout();
		} catch {
			this.eveScout = [];
		}
	}
	connections = $derived(this.data?.connections ?? []);
	nodeH = $derived(2 * this.grid.cell_size);

	// Connections on the active route: endpoints at adjacent path indices (legacy rule).
	routeConnectionIds = $derived.by(() => {
		const out = new Set<number>();
		if (this.routePath.length < 2) return out;
		const index = new Map<number, number>();
		this.routePath.forEach((id, i) => index.set(id, i));
		const placementSystem = new Map<number, number>();
		for (const s of this.systems) placementSystem.set(s.id, s.solar_system_id);
		for (const c of this.connections) {
			const a = index.get(placementSystem.get(c.from_system) ?? -1);
			const b = index.get(placementSystem.get(c.to_system) ?? -1);
			if (a !== undefined && b !== undefined && Math.abs(a - b) === 1) out.add(c.id);
		}
		return out;
	});

	// Position lookup: live drag wins; then an optimistic override; then the server position.
	positions = $derived.by(() => {
		const out = new Map<number, { x: number; y: number }>();
		const dragged = new Map<number, { x: number; y: number }>();
		if (this.drag) {
			const d = this.drag;
			const start = d.members.find((m) => m.id === d.primary);
			const dx = d.x - (start?.sx ?? d.x);
			const dy = d.y - (start?.sy ?? d.y);
			for (const m of d.members) dragged.set(m.id, { x: m.sx + dx, y: m.sy + dy });
		}
		for (const s of this.systems) {
			out.set(
				s.id,
				dragged.get(s.id) ?? this.pending[s.id] ?? { x: s.position_x, y: s.position_y }
			);
		}
		return out;
	});

	constructor(mapId: number) {
		this.mapId = mapId;
	}

	/** Presence: fails silently for viewers (403) and anonymous races. */
	async fetchCharacters() {
		try {
			this.characters = await api.mapCharacters(this.mapId);
		} catch {
			this.characters = [];
		}
	}

	async loadMyCharacters() {
		try {
			this.myCharacters = await api.myCharacters();
		} catch {
			this.myCharacters = [];
		}
	}

	async loadUserSettings() {
		try {
			this.userSettings = await api.mapUserSettings(this.mapId);
		} catch {
			// no access yet; leave null
		}
	}

	async loadGrid() {
		try {
			this.grid = await api.gridConfig();
		} catch {
			// keep the defaults
		}
	}

	async refetch() {
		try {
			const [data, sigs, watchlist] = await Promise.all([
				api.fetchMap(this.mapId),
				api.listSignatures(this.mapId),
				api.listWatchlist(this.mapId)
			]);
			this.watchlist = watchlist;
			this.data = data;
			this.sigs = sigs;
			// Reconcile optimistic move overrides: drop one once the server position matches
			// it (our move landed) or the system is gone.
			const pending = { ...this.pending };
			for (const [id, p] of Object.entries(pending)) {
				const s = data.systems.find((s) => s.id === Number(id));
				if (!s || (Math.abs(s.position_x - p.x) <= 0.5 && Math.abs(s.position_y - p.y) <= 0.5)) {
					delete pending[Number(id)];
				}
			}
			this.pending = pending;
		} catch (err) {
			this.statusLine = `load: ${(err as Error).message}`;
		}
	}

	/**
	 * Run an API call, report the outcome in the status line, and refetch (the WS event also
	 * arrives — both are idempotent).
	 */
	run(label: string, promise: Promise<unknown>) {
		promise
			.then(() => {
				this.statusLine = `${label}: ok`;
				this.refetch();
			})
			.catch((err) => {
				this.statusLine = `${label}: ${(err as Error).message}`;
			});
	}

	// --- geometry ---

	viewportRect(): { left: number; top: number; width: number; height: number } {
		const r = this.viewportEl?.getBoundingClientRect();
		return r
			? { left: r.left, top: r.top, width: r.width, height: r.height }
			: { left: 0, top: 0, width: 1200, height: 1400 };
	}

	/** Screen (client) point → world coords, accounting for pan + zoom. */
	toWorld(clientX: number, clientY: number): { x: number; y: number } {
		const r = this.viewportRect();
		return {
			x: (clientX - r.left - this.pan.x) / this.zoom,
			y: (clientY - r.top - this.pan.y) / this.zoom
		};
	}

	zoomBy(factor: number) {
		const z = this.zoom;
		const nz = clamp(z * factor, 0.25, 3);
		// Keep the viewport center fixed while zooming.
		const r = this.viewportRect();
		const cx = r.width / 2;
		const cy = r.height / 2;
		const wx = (cx - this.pan.x) / z;
		const wy = (cy - this.pan.y) / z;
		this.pan = { x: cx - wx * nz, y: cy - wy * nz };
		this.zoom = nz;
	}

	snap(v: number): number {
		return Math.round(v / this.grid.cell_size) * this.grid.cell_size;
	}

	clampNodeX(x: number): number {
		return clamp(x, 0, this.grid.world_width - NODE_W);
	}

	clampNodeY(y: number): number {
		return clamp(y, 0, this.grid.world_height - this.nodeH);
	}

	openMenu(x: number, y: number, target: MenuTarget) {
		this.menu = { x, y, target };
	}

	closeMenu() {
		this.menu = null;
	}
}
