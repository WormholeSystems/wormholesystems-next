// The map page's shared state. Interaction state is owned here, never derived from the
// fetched data, so it survives refetches.

import { api } from '$lib/api/client';
import type { GridConfig } from '$lib/api/types/GridConfig';
import type { CharacterRef } from '$lib/api/types/CharacterRef';
import type { MapCharacter } from '$lib/api/types/MapCharacter';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
import type { UpdateMapUserSettings } from '$lib/api/types/UpdateMapUserSettings';
import type { MapView } from '$lib/api/types/MapView';
import type { EveScoutConnection } from '$lib/api/types/EveScoutConnection';
import type { Signature } from '$lib/api/types/Signature';
import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
import type { MapHistory } from '$lib/api/types/MapHistory';
import type { StaleConnection } from '$lib/api/types/StaleConnection';
import type { WatchlistEntry } from '$lib/api/types/WatchlistEntry';
import type { SocketState } from '$lib/ws';
import type { BreakpointKey, PanelId, PanelLayouts } from './panels/registry';
import { DEFAULT_LAYOUTS, placeAtBottom, resolveLayouts } from './panels/registry';
import type { GridItem } from '$lib/layout/grid';
import type {
	DynamicEdge,
	RouteGraph,
	RouteStep,
	RoutingSettings
} from '$lib/routing/algorithm';
import { buildDynamicAdjacency } from '$lib/routing/algorithm';
import type { MassStatus } from '$lib/api/types/MassStatus';
import type { TimeStatus } from '$lib/api/types/TimeStatus';
import { NODE_W, clamp } from '$lib/map/helpers';
import { freeEdges, treeEdges, type EdgeGeometry } from '$lib/map/edges';
import { compareForTree, computeTreeLayout } from '$lib/map/tree';
import { orphanedSystems } from '$lib/map/orphans';
import { browser } from '$app/environment';
import { toast } from 'svelte-sonner';
import { MAP_ACTIONS, type MapAction } from './actions';
import type { MapEventEntry } from '$lib/api/types/MapEventEntry';
import { timeAgo } from '$lib/format';

/** Half size is where node text stops being readable, double where a chain stops fitting. */
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 2;
const ZOOM_STEP = 0.1;

/** How long the scrollbars stay up after the last thing that moved the view. */
const SCROLLBAR_LINGER_MS = 1500;

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
	eveScout = $state<EveScoutConnection[]>([]);
	characters = $state<MapCharacter[]>([]);
	myCharacters = $state<CharacterRef[]>([]);
	userSettings = $state<MapUserSettings | null>(null);


	pan = $state({ x: 0, y: 0 });
	zoom = $state(1);
	/** Shown while the scrollbars are awake; they fade out once nothing has moved. */
	scrollbarsVisible = $state(false);
	selected = $state<Set<number>>(new Set());
	drag = $state<Drag | null>(null);
	// Optimistic positions held from drop until the server confirms them, so a moved node
	// doesn't flash back to its old spot during the refetch round-trip.
	pending = $state<Record<number, { x: number; y: number }>>({});
	linking = $state<Linking | null>(null);
	band = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
	menu = $state<Menu | null>(null);
	panDrag = $state<{ cx: number; cy: number; px: number; py: number } | null>(null);
	paletteOpen = $state(false);
	/** The "clean map" confirmation, opened from the status bar hint or the map menu. */
	cleanPrompt = $state(false);
	// Layout edit mode, the breakpoint being edited, and the working copy of the
	// arrangement. The draft is what the grid renders, so a drag shows immediately; it is
	// only persisted on Save, which is what makes Discard possible.
	editingLayout = $state(false);
	/** Raised when leaving edit mode with unsaved changes, so nothing is lost silently. */
	layoutExitPrompt = $state(false);
	layoutBreakpoint = $state<BreakpointKey>('lg');
	layoutDraft = $state<PanelLayouts | null>(null);
	/** The last saved arrangement, for dirty-tracking and for reverting to.  */
	layoutSaved = $state<PanelLayouts | null>(null);
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
	// When set, a search pick says which system this ghost placement turned out to be,
	// instead of placing a new one.
	assignGhostId = $state<number | null>(null);
	// Route planner: origin/destination (solar system ids) and the computed path, set by
	// the navigation card. The path drives the edge highlight.
	routeFromId = $state<number | null>(null);
	routeToId = $state<number | null>(null);
	routePath = $state<number[]>([]);
	// A row hovered in a side panel: the node it names lights up, and its route temporarily
	// replaces the pinned A→B highlight. Owned here so any card can point at the map.
	hoveredSystemId = $state<number | null>(null);
	hoverPath = $state<number[] | null>(null);
	// Systems the router steers around (per map, persisted locally).
	ignoredSystems = $state<Set<number>>(new Set());
	// Display data for systems a side panel names but the map does not hold: a pilot in
	// known space, a skyhook out in sov null. Fetched once each and kept, because the
	// context menu needs the same shape wherever a system is shown.
	resolvedSystems = $state<Map<number, SystemSearchResult>>(new Map());
	// The static routing data, fetched once and shared: the navigation card plans routes
	// with it, and the pilots card measures distances with it. One home, one fetch.
	stargates = $state<Map<number, number[]> | null>(null);
	security = $state<Map<number, number>>(new Map());
	joveSystems = $state<Set<number>>(new Set());
	stationSystems = $state<Set<number>>(new Set());
	serviceOptions = $state<
		{
			id: number;
			name: string;
			systems: Set<number>;
			/** Concrete stations per system, so results can name (and target) the station. */
			stationsBySystem: Map<number, { id: number; name: string }[]>;
		}[]
	>([]);

	// The history tree plus where the map sits in it, and the live socket state behind
	// the status dot.
	history = $state<MapHistory | null>(null);
	/** Bumped when a kill lands in one of this map's systems, so cards can refetch. */
	killmailTick = $state(0);
	// Connections critical for over an hour, offered for a one-click sweep.
	stale = $state<StaleConnection[]>([]);
	socket = $state<SocketState>('connecting');
	/** The canvas's rendered size, kept current by a ResizeObserver on the viewport. */
	viewportSize = $state({ width: 1200, height: 1400 });
	// The page holds a loader until both the graph and the arrangement have arrived, so
	// tiles are never painted in the built-in positions and then moved.
	loaded = $state(false);
	settingsLoaded = $state(false);
	loadError = $state('');
	ready = $derived(this.loaded && this.settingsLoaded);

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

	routingSettings = $derived<RoutingSettings>({
		preference: (this.userSettings?.route_preference ??
			'shorter') as RoutingSettings['preference'],
		securityPenalty: this.userSettings?.security_penalty ?? 50,
		allowTimeStatus: (this.userSettings?.route_allow_time_status ?? 'critical') as TimeStatus,
		allowMassStatus: (this.userSettings?.route_allow_mass_status ?? 'reduced') as MassStatus
	});
	useEveScout = $derived(this.userSettings?.route_use_evescout ?? false);

	/** Stargates plus the chain's own edges. `null` until the static data has arrived. */
	graph = $derived.by<RouteGraph | null>(() => {
		const stargates = this.stargates;
		if (!stargates) return null;
		// Ghosts are left out: a hole whose far side is unknown leads nowhere the router
		// could take you.
		const placementSystem = new Map<number, number>();
		for (const s of this.systems) {
			if (s.solar_system_id !== null) placementSystem.set(s.id, s.solar_system_id);
		}
		const edges: DynamicEdge[] = [];
		for (const c of this.connections) {
			if (c.kind !== 'wormhole') continue;
			const a = placementSystem.get(c.from_system);
			const b = placementSystem.get(c.to_system);
			if (a === undefined || b === undefined || a === b) continue;
			edges.push({ a, b, via: 'wormhole', mass: c.mass_status, time: c.time_status });
		}
		if (this.useEveScout) {
			for (const e of this.eveScout) {
				edges.push({
					a: e.hub_solar_system_id,
					b: e.solar_system_id,
					via: 'evescout',
					mass: e.mass_status as MassStatus,
					time: e.time_status as TimeStatus
				});
			}
		}
		return { stargates, dynamic: buildDynamicAdjacency(edges), security: this.security };
	});

	// Undo and redo move the map's cursor through the history tree rather than recording
	// anything, so the server is the only thing that decides whether they are available.
	entries = $derived(this.history?.entries ?? []);
	canUndo = $derived(this.history?.can_undo ?? false);
	canRedo = $derived(this.history?.can_redo ?? false);
	/** The step the map is sitting on, for labelling the undo button. */
	headEntry = $derived(this.entries.find((e) => e.id === this.history?.head_event_id) ?? null);
	redoEntry = $derived(this.entries.find((e) => e.id === this.history?.redo_target) ?? null);

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
		for (const s of this.systems) {
			if (s.solar_system_id !== null) placementSystem.set(s.id, s.solar_system_id);
		}
		for (const c of this.connections) {
			const a = index.get(placementSystem.get(c.from_system) ?? -1);
			const b = index.get(placementSystem.get(c.to_system) ?? -1);
			if (a !== undefined && b !== undefined && Math.abs(a - b) === 1) out.add(c.id);
		}
		return out;
	});

	/**
	 * How this map is placed for this viewer: the map's own mode, unless it hands the
	 * choice over and this viewer has made one.
	 */
	layout = $derived.by<'manual' | 'tree'>(() => {
		const map = this.data?.map;
		if (!map) return 'manual';
		const own = this.userSettings?.layout_override;
		if (map.allow_layout_override && (own === 'manual' || own === 'tree')) return own;
		return map.layout === 'tree' ? 'tree' : 'manual';
	});

	/** Automatic placement owns the positions, so dragging one would fight the layout. */
	layoutLocked = $derived(this.layout === 'tree');

	/** Derived from the shape of the chain, recomputed only when that shape changes. */
	treePositions = $derived.by(() => {
		if (this.layout !== 'tree') return null;
		const systems = new Map(this.systems.map((s) => [s.id, s]));
		return computeTreeLayout(
			{
				nodeIds: this.systems.map((s) => s.id),
				edges: this.connections.map((c) => ({ from: c.from_system, to: c.to_system })),
				rootIds: this.systems.filter((s) => s.is_pinned).map((s) => s.id),
				homeId: this.systems.find((s) => s.is_home)?.id ?? null,
				fallbackRootId: null,
				compareNodes: compareForTree(systems)
			},
			{ gridSize: this.grid.cell_size }
		);
	});

	/**
	 * The scanner id an unmapped hole is known by. A ghost has no name of its own, so the
	 * signature its connection is linked to is the only thing that identifies it.
	 */
	ghostSignatures = $derived.by(() => {
		const out = new Map<number, string>();
		const ghosts = new Set(
			this.systems.filter((s) => s.solar_system_id === null).map((s) => s.id)
		);
		if (ghosts.size === 0) return out;
		const byConnection = new Map(
			this.sigs.filter((s) => s.connection_id !== null).map((s) => [s.connection_id!, s])
		);
		for (const c of this.connections) {
			const sig = byConnection.get(c.id);
			if (!sig) continue;
			if (ghosts.has(c.to_system)) out.set(c.to_system, sig.signature_id);
			if (ghosts.has(c.from_system)) out.set(c.from_system, sig.signature_id);
		}
		return out;
	});

	/** Every connection's line and badge anchor, routed the way this layout draws them. */
	edgeGeometry = $derived.by<Map<number, EdgeGeometry>>(() =>
		this.layout === 'tree'
			? treeEdges(this.connections, this.positions, this.nodeH)
			: freeEdges(this.connections, this.positions, this.nodeH)
	);

	// Position lookup: the automatic layout when one is active; else live drag, then an
	// optimistic override, then the server position.
	positions = $derived.by(() => {
		const tree = this.treePositions;
		if (tree) return tree;
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

	/**
	 * False for somebody watching through a share link or a public map: everything hung off
	 * an account is not fetched rather than fetched and refused.
	 */
	signedIn = $state(true);

	constructor(mapId: number, signedIn = true, seed: MapView | null = null) {
		this.mapId = mapId;
		this.signedIn = signedIn;
		// Seeded from the page's load, so the first frame already knows the chain. The
		// refetch still runs: this is a starting point, not the last word.
		if (seed) {
			this.data = seed;
			this.loaded = true;
		}
	}

	/** Panels there is nobody to fill in: both of these are about the account, not the map. */
	unavailablePanels = $derived(
		new Set<PanelId>(this.signedIn ? [] : ['characters', 'skyhooks'])
	);

	/** Presence: fails silently for viewers (403) and anonymous races. */
	async fetchCharacters() {
		if (!this.signedIn) return;
		try {
			this.characters = await api.mapCharacters(this.mapId);
		} catch {
			this.characters = [];
		}
	}

	async loadMyCharacters() {
		if (!this.signedIn) return;
		try {
			this.myCharacters = await api.myCharacters();
		} catch {
			this.myCharacters = [];
		}
	}

	/**
	 * A system in the shape every picker and the context menu expect, whether or not it is
	 * on the map. Returns null until [`ensureResolved`] has fetched an off-map one.
	 */
	systemInfo(solarSystemId: number): SystemSearchResult | null {
		const placed = this.systems.find(
			(s) => s.solar_system_id === solarSystemId && s.name !== null
		);
		if (placed) {
			return {
				id: solarSystemId,
				name: placed.name ?? '',
				security: placed.security_status ?? 0,
				region: placed.region ?? '',
				region_id: placed.region_id ?? 0,
				constellation_id: placed.constellation_id ?? 0,
				wormhole_class_id: placed.wormhole_class_id,
				effect_name: placed.effect_name,
				sovereignty: placed.sovereignty,
				statics: placed.statics
			};
		}
		return this.resolvedSystems.get(solarSystemId) ?? null;
	}

	/** Fetch display data for any of `ids` that is neither on the map nor already known. */
	ensureResolved(ids: number[]) {
		const placed = new Set(this.systems.map((s) => s.solar_system_id).filter((id) => id !== null));
		const missing = [
			...new Set(ids.filter((id) => !placed.has(id) && !this.resolvedSystems.has(id)))
		];
		if (missing.length === 0) return;
		api
			.resolveSystems(missing)
			.then((rows) => {
				const next = new Map(this.resolvedSystems);
				for (const row of rows) next.set(row.id, row);
				this.resolvedSystems = next;
			})
			.catch(() => {});
	}

	/** The static routing tables (stargates, security, Jove/station/service indexes). */
	private routingLoad: Promise<void> | null = null;

	/**
	 * Resolves once the static routing tables are in. Jump tracking waits on this rather
	 * than reading `stargates` directly, since a jump taken seconds after the map opens
	 * would otherwise be judged against an empty gate table.
	 */
	whenRoutingLoaded(): Promise<void> {
		return this.routingLoad ?? this.loadRoutingGraph();
	}

	async loadRoutingGraph() {
		this.routingLoad ??= this.fetchRoutingGraph();
		return this.routingLoad;
	}

	private async fetchRoutingGraph() {
		try {
			const g = await api.routingGraph();
			this.stargates = new Map(
				Object.entries(g.adjacency).map(([k, v]) => [Number(k), v as number[]])
			);
			this.security = new Map(Object.entries(g.security).map(([k, v]) => [Number(k), v]));
			this.joveSystems = new Set(g.jove ?? []);
			this.stationSystems = new Set(g.stations ?? []);
			this.serviceOptions = (g.services ?? []).map((svc) => {
				const stationsBySystem = new Map<number, { id: number; name: string }[]>();
				for (const station of svc.stations) {
					const list = stationsBySystem.get(station.solar_system_id) ?? [];
					list.push({ id: station.id, name: station.name });
					stationsBySystem.set(station.solar_system_id, list);
				}
				return {
					id: svc.id,
					name: svc.name,
					systems: new Set(stationsBySystem.keys()),
					stationsBySystem
				};
			});
		} catch {
			// No graph means no routing; the cards fall back to showing no distances.
		}
	}

	/** Bumped by every write, so a fetch that started earlier cannot land afterwards. */
	private settingsVersion = 0;

	async loadUserSettings() {
		if (!this.signedIn) {
			this.settingsLoaded = true;
			return;
		}
		const version = this.settingsVersion;
		try {
			const settings = await api.mapUserSettings(this.mapId);
			this.layoutSaved = settings.layout_breakpoints ?? null;
			this.layoutDraft = structuredClone($state.snapshot(this.layoutSaved));
			if (version !== this.settingsVersion) return;
			this.userSettings = settings;
		} catch {
			// No access yet; the page falls back to the built-in arrangement.
		} finally {
			this.settingsLoaded = true;
		}
	}

	// --- layout ---

	private hiddenDirty = $state(false);
	layoutDirty = $derived(
		JSON.stringify(this.layoutDraft) !== JSON.stringify(this.layoutSaved) || this.hiddenDirty
	);

	/**
	 * `items` only covers the visible panels; the hidden ones are carried over, since their
	 * stored positions are what put them back when they are unhidden.
	 */
	setLayoutItems(key: BreakpointKey, items: GridItem[]) {
		const base = resolveLayouts(this.layoutDraft);
		const hidden = new Set(this.userSettings?.hidden_panels ?? []);
		const kept = base[key].items.filter((i) => hidden.has(i.i));
		this.layoutDraft = { ...base, [key]: { ...base[key], items: [...items, ...kept] } };
	}

	setLayout(layouts: PanelLayouts) {
		this.layoutDraft = layouts;
	}

	hidePanel(id: string) {
		if (!this.userSettings || this.userSettings.hidden_panels.includes(id)) return;
		this.userSettings = {
			...this.userSettings,
			hidden_panels: [...this.userSettings.hidden_panels, id]
		};
		this.hiddenDirty = true;
	}

	showPanel(id: string) {
		if (!this.userSettings) return;
		// Put it back at the bottom of every breakpoint, so unhiding never drops a tile
		// into a hole left by something that has since moved.
		const base = resolveLayouts(this.layoutDraft);
		for (const key of Object.keys(base)) {
			base[key] = placeAtBottom(base[key], id as PanelId);
		}
		this.layoutDraft = base;
		this.userSettings = {
			...this.userSettings,
			hidden_panels: this.userSettings.hidden_panels.filter((p) => p !== id)
		};
		this.hiddenDirty = true;
	}

	saveLayout() {
		const layouts = resolveLayouts(this.layoutDraft);
		this.run(
			'saveLayout',
			this.patchUserSettings({
				layout_breakpoints: layouts,
				hidden_panels: this.userSettings?.hidden_panels ?? []
			}).then((saved) => {
				this.layoutSaved = saved.layout_breakpoints ?? null;
				this.layoutDraft = structuredClone($state.snapshot(this.layoutSaved));
				this.hiddenDirty = false;
				this.editingLayout = false;
			})
		);
	}

	/** Leave arrange mode; unsaved changes raise the prompt instead of vanishing. */
	exitLayoutEdit() {
		if (this.layoutDirty) {
			this.layoutExitPrompt = true;
			return;
		}
		this.editingLayout = false;
	}

	/** Answer the prompt: keep the changes, or throw them away. */
	resolveLayoutExit(save: boolean) {
		this.layoutExitPrompt = false;
		if (save) {
			this.saveLayout();
			return;
		}
		this.revertLayout();
		this.editingLayout = false;
	}

	/** Throw the working copy away and go back to what was last saved. */
	revertLayout() {
		this.layoutDraft = structuredClone($state.snapshot(this.layoutSaved));
		if (this.userSettings) {
			this.userSettings = {
				...this.userSettings,
				hidden_panels: this.layoutSavedHidden
			};
		}
		this.hiddenDirty = false;
	}

	/** Hidden panels as of the last save, so a revert restores them too. */
	private layoutSavedHidden: string[] = [];

	rememberHidden() {
		this.layoutSavedHidden = [...(this.userSettings?.hidden_panels ?? [])];
	}

	/** Put one breakpoint back to the built-in arrangement. */
	resetLayout(key: BreakpointKey) {
		const base = resolveLayouts(this.layoutDraft);
		this.layoutDraft = { ...base, [key]: structuredClone(DEFAULT_LAYOUTS[key]) };
	}

	async loadGrid() {
		try {
			this.grid = await api.gridConfig();
		} catch {
			// keep the defaults
		}
	}

	async fetchStale() {
		if (!this.signedIn) return;
		try {
			this.stale = await api.listStaleConnections(this.mapId);
		} catch {
			this.stale = [];
		}
	}

	cleanStale() {
		this.run('cleanStale', api.cleanStaleConnections({ map_id: this.mapId }));
	}

	async fetchHistory() {
		if (!this.signedIn) return;
		try {
			this.history = await api.mapHistory(this.mapId);
		} catch {
			this.history = null;
		}
	}

	// Undo and redo are read before they run: the step being walked past is the head now,
	// and after the call it is somewhere else. "Undone" alone leaves you to work out what
	// went, which on a map somebody else is also editing is not obvious.
	undo() {
		this.run('undo', api.undoMapEvent(this.mapId), this.stepDetail(this.headEntry));
	}

	redo() {
		this.run('redo', api.redoMapEvent(this.mapId), this.stepDetail(this.redoEntry));
	}

	/** Jump the map to any step, which is how a branch left behind by an undo is re-entered. */
	gotoEvent(eventId: number | null) {
		const target = this.entries.find((e) => e.id === eventId) ?? null;
		this.run(
			'goToEvent',
			api.gotoMapEvent({ map_id: this.mapId, event_id: eventId }),
			eventId === null ? 'Back to the empty map' : this.stepDetail(target)
		);
	}

	async refetch() {
		try {
			// All five go out together, but the page only waits on the graph: the panels can
			// fill in a moment later, and holding first paint for every list makes the map
			// feel slow for no benefit.
			const graph = api.fetchMap(this.mapId);
			const sigs = api.listSignatures(this.mapId);
			const watchlist = api.listWatchlist(this.mapId);
			const history = this.fetchHistory();
			const stale = this.fetchStale();

			const data = await graph;
			this.data = data;
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
			this.loaded = true;
			this.loadError = '';

			this.sigs = await sigs;
			this.watchlist = await watchlist;
			await history;
			await stale;
		} catch (err) {
			const message = (err as Error).message;
			toast.error(`load: ${message}`);
			// Only the first load can leave the page with nothing to show; a later failure
			// just means the view is briefly stale.
			if (!this.loaded) this.loadError = message;
		}
	}

	/**
	 * Run an action and refetch (the WS event also arrives; both are idempotent). What it
	 * says on the way out lives in [`MAP_ACTIONS`] rather than at the call site.
	 */
	run(action: MapAction, promise: Promise<unknown>, detail?: string) {
		const copy = MAP_ACTIONS[action];
		promise
			.then(() => {
				if ('done' in copy && copy.done) {
					toast.success(copy.done, { description: detail });
				}
				return this.refetch();
			})
			.catch((err) => toast.error(copy.failed, { description: (err as Error).message }));
	}

	/** What a history step was, for saying which one was just walked past. */
	private stepDetail(entry: MapEventEntry | null): string | undefined {
		if (!entry) return undefined;
		return `${entry.label} · ${timeAgo(entry.created_at)}`;
	}

	/**
	 * The signature to warp to for a wormhole hop. A connection has one at each end; the
	 * one that matters is on the side you are leaving, which is the one in your scanner.
	 */
	wormholeSignature(from: number, to: number): string | null {
		const system = new Map(this.systems.map((s) => [s.id, s.solar_system_id]));
		const conn = this.connections.find((c) => {
			const a = system.get(c.from_system);
			const b = system.get(c.to_system);
			return (a === from && b === to) || (a === to && b === from);
		});
		if (!conn) return null;
		return (
			this.sigs.find((sig) => sig.connection_id === conn.id && sig.solar_system_id === from)
				?.signature_id ?? null
		);
	}

	/** Route steps with the signature attached to each wormhole hop. */
	withSignatures(steps: RouteStep[]): (RouteStep & { signature: string | null })[] {
		return steps.map((step, i) => ({
			...step,
			signature:
				step.via === 'wormhole' && i > 0 ? this.wormholeSignature(steps[i - 1].id, step.id) : null
		}));
	}

	// --- geometry ---

	viewportRect(): { left: number; top: number; width: number; height: number } {
		const r = this.viewportEl?.getBoundingClientRect();
		return {
			// Position is read live: it only matters during a pointer event, and it moves
			// with scrolling rather than with the element's own size.
			left: r?.left ?? 0,
			top: r?.top ?? 0,
			// Size comes from the observer instead of the rect, because a rect read is not
			// reactive: anything derived from it (the scrollbar thumbs) would keep the
			// value it had when the canvas was first measured.
			width: this.viewportSize.width,
			height: this.viewportSize.height
		};
	}

	/** Screen (client) point → world coords, accounting for pan + zoom. */
	toWorld(clientX: number, clientY: number): { x: number; y: number } {
		const r = this.viewportRect();
		return {
			x: (clientX - r.left - this.pan.x) / this.zoom,
			y: (clientY - r.top - this.pan.y) / this.zoom
		};
	}

	/**
	 * Pick a placement for yourself. Choosing the map's own mode clears the override
	 * rather than pinning the same value, so a later change to the map still reaches you.
	 */
	setLayoutOverride(mode: 'manual' | 'tree') {
		const own = mode === this.data?.map.layout ? null : mode;
		if (this.userSettings) {
			this.userSettings = { ...this.userSettings, layout_override: own ?? undefined };
		}
		this.patchUserSettings({ layout_override: own }).catch((err) =>
			toast.error(`placement: ${(err as Error).message}`)
		);
	}

	/**
	 * Write one or more of the viewer's own settings. Everything goes through here so the
	 * version guard covers every write, not just the one that happened to have it.
	 */
	async patchUserSettings(patch: UpdateMapUserSettings): Promise<MapUserSettings> {
		this.settingsVersion++;
		const saved = await api.updateMapUserSettings(this.mapId, patch);
		this.userSettings = saved;
		return saved;
	}

	orphaned = $derived(orphanedSystems(this.systems, this.connections));

	/** Take the dead branches off the map. */
	cleanMap() {
		const ids = this.orphaned.map((s) => s.id);
		if (ids.length === 0) return;
		this.run(
			'cleanMap',
			api.removeSystems({ map_id: this.mapId, map_solar_system_ids: ids }),
			`${ids.length} ${ids.length === 1 ? 'system' : 'systems'}`
		);
	}

	/** Shift the view by a screen-pixel delta (wheel, scrollbar, drag). */
	panBy(dx: number, dy: number) {
		this.pan = { x: this.pan.x + dx, y: this.pan.y + dy };
		this.wakeScrollbars();
	}

	/** Shown while the view is moving or the cursor is over the canvas, then faded out. */
	wakeScrollbars() {
		this.scrollbarsVisible = true;
		if (this.hideScrollbars) clearTimeout(this.hideScrollbars);
		this.hideScrollbars = setTimeout(() => {
			this.scrollbarsVisible = false;
			this.hideScrollbars = null;
		}, SCROLLBAR_LINGER_MS);
	}

	private hideScrollbars: ReturnType<typeof setTimeout> | null = null;

	/** Zoom by whole steps, keeping the middle of the viewport where it is. */
	zoomBy(steps: number) {
		const next = Math.round((this.zoom + steps * ZOOM_STEP) * 10) / 10;
		const nz = clamp(next, ZOOM_MIN, ZOOM_MAX);
		if (nz === this.zoom) return;
		const z = this.zoom;
		const r = this.viewportRect();
		const cx = r.width / 2;
		const cy = r.height / 2;
		const wx = (cx - this.pan.x) / z;
		const wy = (cy - this.pan.y) / z;
		this.pan = { x: cx - wx * nz, y: cy - wy * nz };
		this.zoom = nz;
		this.rememberZoom();
		this.wakeScrollbars();
	}

	/** Per map and per browser: how far out you want to be depends on the screen. */
	restoreZoom() {
		if (!browser) return;
		const saved = Number(localStorage.getItem(`map-zoom-${this.mapId}`));
		if (saved >= ZOOM_MIN && saved <= ZOOM_MAX) this.zoom = saved;
	}

	private rememberZoom() {
		if (browser) localStorage.setItem(`map-zoom-${this.mapId}`, String(this.zoom));
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
