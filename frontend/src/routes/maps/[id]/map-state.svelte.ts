// The map page's shared state. Interaction state is owned here, never derived from the
// fetched data, so it survives refetches.

import { api, errorMessage } from '$lib/api/client';
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
import type { GridItem } from '$lib/layout/grid';
import type { RouteStep, RoutingSettings } from '$lib/routing/algorithm';
import { NODE_W, clamp } from '$lib/map/helpers';
import { freeEdges, treeEdges, type EdgeGeometry } from '$lib/map/edges';
import { draggedPositions, type Drag } from '$lib/map/gestures';
import type { Vec2 } from '$lib/map/helpers';
import { compareForTree, computeTreeLayout } from '$lib/map/tree';
import { orphanedSystems } from '$lib/map/orphans';
import { toast } from 'svelte-sonner';
import { MAP_ACTIONS, type MapAction } from './actions';
import { MapCamera } from './map-camera.svelte';
import { PanelLayoutStore } from './panel-layout.svelte';
import { RoutePlanner } from './route-planner.svelte';
import { SliceFetcher, slicesFor, type Slice } from './slices.svelte';
import type { MapEvent } from '$lib/api/types/MapEvent';
import type { MapEventEntry } from '$lib/api/types/MapEventEntry';
import { timeAgo } from '$lib/format';
import { solarSystemId } from '$lib/map/system';
import { atLeast } from '$lib/map/roles';

export type { Drag };

/** An in-progress connection drag: from this placement to the current cursor (world coords). */
export interface Linking {
	from: number;
	x: number;
	y: number;
}

export type MenuTarget =
	{ kind: 'map' } | { kind: 'node'; system: MapSystemView } | { kind: 'connection'; id: number };

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
	viewport_height: 1400,
};

export class MapState {
	mapId: number;
	camera: MapCamera;
	panelLayout: PanelLayoutStore;
	route: RoutePlanner;

	data = $state<MapView | null>(null);
	grid = $state<GridConfig>(defaultGrid);
	sigs = $state<Signature[]>([]);
	watchlist = $state<WatchlistEntry[]>([]);
	eveScout = $state<EveScoutConnection[]>([]);
	characters = $state<MapCharacter[]>([]);
	myCharacters = $state<CharacterRef[]>([]);
	userSettings = $state<MapUserSettings | null>(null);

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
	// A row hovered in a side panel: the node it names lights up. Owned here so any card
	// can point at the map.
	hoveredSystemId = $state<number | null>(null);
	// Display data for systems a side panel names but the map does not hold: a pilot in
	// known space, a skyhook out in sov null. Fetched once each and kept, because the
	// context menu needs the same shape wherever a system is shown.
	resolvedSystems = $state<Map<number, SystemSearchResult>>(new Map());

	// The history tree plus where the map sits in it, and the live socket state behind
	// the status dot.
	history = $state<MapHistory | null>(null);
	/** Bumped when a kill lands in one of this map's systems, so cards can refetch. */
	killmailTick = $state(0);
	// Connections critical for over an hour, offered for a one-click sweep.
	stale = $state<StaleConnection[]>([]);
	socket = $state<SocketState>('connecting');
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
			(this.activeSystem ? solarSystemId(this.activeSystem) : null) ??
			this.myCharacters.find((c) => c.is_active && c.online)?.solar_system_id ??
			this.myCharacters.find((c) => c.online && c.solar_system_id !== null)?.solar_system_id ??
			null,
	);

	routingSettings = $derived<RoutingSettings>({
		preference: this.userSettings?.route_preference ?? 'shorter',
		securityPenalty: this.userSettings?.security_penalty ?? 50,
		allowTimeStatus: this.userSettings?.route_allow_time_status ?? 'critical',
		allowMassStatus: this.userSettings?.route_allow_mass_status ?? 'reduced',
	});
	useEveScout = $derived(this.userSettings?.route_use_evescout ?? false);

	// Undo and redo move the map's cursor through the history tree rather than recording
	// anything, so the server is the only thing that decides whether they are available.
	entries = $derived(this.history?.entries ?? []);
	canUndo = $derived(this.history?.can_undo ?? false);
	canRedo = $derived(this.history?.can_redo ?? false);
	/** The step the map is sitting on, for labelling the undo button. */
	headEntry = $derived(this.entries.find((e) => e.id === this.history?.head_event_id) ?? null);
	redoEntry = $derived(this.entries.find((e) => e.id === this.history?.redo_target) ?? null);

	async loadEveScout() {
		try {
			this.eveScout = await api.eveScout();
		} catch {
			this.eveScout = [];
		}
	}

	/** EVE Scout is scouted by hand, so it changes on the order of minutes at best. */
	startEveScoutPolling(): () => void {
		this.loadEveScout();
		const timer = setInterval(() => this.loadEveScout(), 5 * 60_000);
		return () => clearInterval(timer);
	}
	connections = $derived(this.data?.connections ?? []);
	nodeH = $derived(2 * this.grid.cell_size);

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
				compareNodes: compareForTree(systems),
			},
			{ gridSize: this.grid.cell_size },
		);
	});

	/**
	 * The scanner id an unmapped hole is known by. A ghost has no name of its own, so the
	 * signature its connection is linked to is the only thing that identifies it.
	 */
	ghostSignatures = $derived.by(() => {
		const out = new Map<number, string>();
		const ghosts = new Set(this.systems.filter((s) => s.kind === 'ghost').map((s) => s.id));
		if (ghosts.size === 0) return out;
		const byConnection = new Map(
			this.sigs.filter((s) => s.connection_id !== null).map((s) => [s.connection_id!, s]),
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
			: freeEdges(this.connections, this.positions, this.nodeH),
	);

	// Position lookup: the automatic layout when one is active; else live drag, then an
	// optimistic override, then the server position.
	positions = $derived.by(() => {
		const tree = this.treePositions;
		if (tree) return tree;
		const out = new Map<number, { x: number; y: number }>();
		const dragged = this.drag ? draggedPositions(this.drag) : new Map<number, Vec2>();
		for (const s of this.systems) {
			out.set(
				s.id,
				dragged.get(s.id) ?? this.pending[s.id] ?? { x: s.position_x, y: s.position_y },
			);
		}
		return out;
	});

	/**
	 * False for somebody watching through a share link or a public map: everything hung off
	 * an account is not fetched rather than fetched and refused.
	 */
	signedIn = $state(true);

	/** Editing takes the member role; below it the map is read-only. */
	canWrite = $derived(atLeast(this.data?.role, 'member'));

	private slices = new SliceFetcher((slice) => this.fetchSlice(slice));

	/** True until the first refetch, so a seeded graph is not fetched twice on open. */
	private seeded = false;

	constructor(
		mapId: number,
		signedIn = true,
		seed: { view: MapView | null; settings: MapUserSettings | null } = {
			view: null,
			settings: null,
		},
	) {
		this.mapId = mapId;
		this.camera = new MapCamera(mapId);
		this.panelLayout = new PanelLayoutStore({
			hiddenPanels: () => this.userSettings?.hidden_panels ?? null,
			setHiddenPanels: (panels) => {
				if (this.userSettings) {
					this.userSettings = { ...this.userSettings, hidden_panels: panels };
				}
			},
			save: (layouts, hidden) => {
				const write = this.patchUserSettings({
					layout_breakpoints: layouts,
					hidden_panels: hidden,
				}).then((saved) => saved.layout_breakpoints ?? null);
				this.run('saveLayout', write);
				return write;
			},
		});
		this.route = new RoutePlanner(this);
		this.signedIn = signedIn;
		// Seeded from the page's load, so the first frame already has the chain and the
		// arrangement. Both are starting points: the socket and the refetch take over.
		if (seed.view) {
			this.data = seed.view;
			this.loaded = true;
			this.seeded = true;
		}
		if (seed.settings) {
			this.userSettings = seed.settings;
			this.panelLayout.seed(seed.settings.layout_breakpoints ?? null);
			this.settingsLoaded = true;
		}
	}

	/** Panels there is nobody to fill in: both of these are about the account, not the map. */
	unavailablePanels = $derived(new Set<PanelId>(this.signedIn ? [] : ['characters', 'skyhooks']));

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
	systemInfo(id: number): SystemSearchResult | null {
		const placed = this.systems.find((s) => solarSystemId(s) === id);
		if (placed?.kind === 'system') {
			return {
				id,
				name: placed.name,
				security: placed.security_status,
				region: placed.region,
				region_id: placed.region_id,
				constellation_id: placed.constellation_id,
				wormhole_class_id: placed.wormhole_class_id,
				effect_name: placed.effect_name,
				sovereignty: placed.sovereignty,
				statics: placed.statics,
			};
		}
		return this.resolvedSystems.get(id) ?? null;
	}

	/** Fetch display data for any of `ids` that is neither on the map nor already known. */
	ensureResolved(ids: number[]) {
		const placed = new Set(this.systems.map(solarSystemId).filter((id) => id !== null));
		const missing = [
			...new Set(ids.filter((id) => !placed.has(id) && !this.resolvedSystems.has(id))),
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

	// --- routing, delegated to the planner ---

	get routeFromId() {
		return this.route.routeFromId;
	}

	set routeFromId(id: number | null) {
		this.route.routeFromId = id;
	}

	get routeToId() {
		return this.route.routeToId;
	}

	set routeToId(id: number | null) {
		this.route.routeToId = id;
	}

	get routePath() {
		return this.route.routePath;
	}

	set routePath(path: number[]) {
		this.route.routePath = path;
	}

	get hoverPath() {
		return this.route.hoverPath;
	}

	set hoverPath(path: number[] | null) {
		this.route.hoverPath = path;
	}

	get ignoredSystems() {
		return this.route.ignoredSystems;
	}

	loadIgnored() {
		this.route.loadIgnored();
	}

	ignoreSystem(id: number) {
		this.route.ignoreSystem(id);
	}

	clearIgnored() {
		this.route.clearIgnored();
	}

	get stargates() {
		return this.route.stargates;
	}

	get security() {
		return this.route.security;
	}

	get joveSystems() {
		return this.route.joveSystems;
	}

	get stationSystems() {
		return this.route.stationSystems;
	}

	get serviceOptions() {
		return this.route.serviceOptions;
	}

	get corporationOptions() {
		return this.route.corporationOptions;
	}

	get graph() {
		return this.route.graph;
	}

	get routeConnectionIds() {
		return this.route.routeConnectionIds;
	}

	whenRoutingLoaded(): Promise<void> {
		return this.route.whenLoaded();
	}

	loadRoutingGraph() {
		return this.route.load();
	}

	withSignatures(steps: RouteStep[]) {
		return this.route.withSignatures(steps);
	}

	/** Bumped by every write, so a fetch that started earlier cannot land afterwards. */
	private settingsVersion = 0;

	async loadUserSettings() {
		// Already seeded by the page's load, or nobody to have settings: either way the page
		// is not waiting on this. A watcher still has to be marked done, or nothing is ready.
		if (this.settingsLoaded) return;
		if (!this.signedIn) {
			this.settingsLoaded = true;
			return;
		}
		const version = this.settingsVersion;
		try {
			const settings = await api.mapUserSettings(this.mapId);
			this.panelLayout.seed(settings.layout_breakpoints ?? null);
			if (version !== this.settingsVersion) return;
			this.userSettings = settings;
		} catch {
			// No access yet; the page falls back to the built-in arrangement.
		} finally {
			this.settingsLoaded = true;
		}
	}

	// --- layout, delegated to the panel layout store ---

	get editingLayout() {
		return this.panelLayout.editing;
	}

	set editingLayout(on: boolean) {
		this.panelLayout.editing = on;
	}

	get layoutExitPrompt() {
		return this.panelLayout.exitPrompt;
	}

	set layoutExitPrompt(on: boolean) {
		this.panelLayout.exitPrompt = on;
	}

	get layoutBreakpoint() {
		return this.panelLayout.breakpoint;
	}

	set layoutBreakpoint(key: BreakpointKey) {
		this.panelLayout.breakpoint = key;
	}

	get layoutDraft() {
		return this.panelLayout.draft;
	}

	get layoutDirty() {
		return this.panelLayout.dirty;
	}

	setLayoutItems(key: BreakpointKey, items: GridItem[]) {
		this.panelLayout.setItems(key, items);
	}

	setLayout(layouts: PanelLayouts) {
		this.panelLayout.set(layouts);
	}

	hidePanel(id: string) {
		this.panelLayout.hidePanel(id);
	}

	showPanel(id: string) {
		this.panelLayout.showPanel(id);
	}

	saveLayout() {
		this.panelLayout.save();
	}

	exitLayoutEdit() {
		this.panelLayout.exitEdit();
	}

	resolveLayoutExit(save: boolean) {
		this.panelLayout.resolveExit(save);
	}

	rememberHidden() {
		this.panelLayout.rememberHidden();
	}

	resetLayout(key: BreakpointKey) {
		this.panelLayout.reset(key);
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
			eventId === null ? 'Back to the empty map' : this.stepDetail(target),
		);
	}

	/** The graph itself, and the optimistic move overrides it settles. */
	private async fetchGraph() {
		// A seeded graph is as fresh as the page: skip the request, once.
		const data = this.seeded ? this.data! : await api.fetchMap(this.mapId);
		this.seeded = false;
		this.data = data;
		// Drop a pending move once the server position matches it (ours landed) or the
		// system is gone.
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
	}

	private async fetchSignatures() {
		this.sigs = await api.listSignatures(this.mapId);
	}

	private async fetchWatchlist() {
		this.watchlist = await api.listWatchlist(this.mapId);
	}

	private async fetchSlice(slice: Slice) {
		try {
			switch (slice) {
				case 'graph':
					return await this.fetchGraph();
				case 'signatures':
					return await this.fetchSignatures();
				case 'watchlist':
					return await this.fetchWatchlist();
				case 'history':
					return await this.fetchHistory();
				case 'stale':
					return await this.fetchStale();
				case 'characters':
					return await this.fetchCharacters();
			}
		} catch (err) {
			const message = errorMessage(err);
			toast.error(`load: ${message}`);
			// Only the first load can leave the page with nothing to show; a later failure
			// just means the view is briefly stale.
			if (!this.loaded) this.loadError = message;
		}
	}

	/** Refetch what one socket frame invalidated; [`slicesFor`] holds the table. */
	applyEvent(event: MapEvent) {
		// A kill changes nothing about the graph; only the killmail card reacts.
		if (event.type === 'killmail_received') {
			this.killmailTick += 1;
			return;
		}
		for (const slice of slicesFor(event)) this.slices.schedule(slice);
	}

	/** Everything, for the first load. After that the socket says what to ask for. */
	async refetch() {
		const rest = [
			this.fetchSlice('signatures'),
			this.fetchSlice('watchlist'),
			this.fetchSlice('history'),
			this.fetchSlice('stale'),
		];
		// The page waits on the graph only: the panels can fill in a moment later, and
		// holding first paint for every list makes the map feel slow for no benefit.
		await this.fetchSlice('graph');
		await Promise.all(rest);
	}

	/**
	 * Run an action and say how it went. What it says lives in [`MAP_ACTIONS`] rather than
	 * at the call site.
	 *
	 * The write is not followed by a refetch while the socket is up: the server echoes the
	 * change back like it does to everyone else, and `applyEvent` asks for exactly the part
	 * that moved. Refetching here as well meant the person doing the editing paid twice for
	 * every write. With the socket down there is no echo, so the fallback stands in.
	 */
	run(action: MapAction, promise: Promise<unknown>, detail?: string) {
		const copy = MAP_ACTIONS[action];
		promise
			.then(() => {
				if ('done' in copy && copy.done) {
					toast.success(copy.done, { description: detail });
				}
				if (this.socket !== 'open') return this.refetch();
			})
			.catch((err) => toast.error(copy.failed, { description: errorMessage(err) }));
	}

	/** What a history step was, for saying which one was just walked past. */
	private stepDetail(entry: MapEventEntry | null): string | undefined {
		if (!entry) return undefined;
		return `${entry.label} · ${timeAgo(entry.created_at)}`;
	}

	// --- geometry, delegated to the camera ---

	get viewportEl() {
		return this.camera.viewportEl;
	}

	set viewportEl(el: HTMLElement | null) {
		this.camera.viewportEl = el;
	}

	get pan() {
		return this.camera.pan;
	}

	set pan(v: Vec2) {
		this.camera.pan = v;
	}

	get zoom() {
		return this.camera.zoom;
	}

	get scrollbarsVisible() {
		return this.camera.scrollbarsVisible;
	}

	get viewportSize() {
		return this.camera.viewportSize;
	}

	set viewportSize(size: { width: number; height: number }) {
		this.camera.viewportSize = size;
	}

	viewportRect() {
		return this.camera.viewportRect();
	}

	toWorld(clientX: number, clientY: number): Vec2 {
		return this.camera.toWorld(clientX, clientY);
	}

	panBy(dx: number, dy: number) {
		this.camera.panBy(dx, dy);
	}

	wakeScrollbars() {
		this.camera.wakeScrollbars();
	}

	zoomBy(steps: number) {
		this.camera.zoomBy(steps);
	}

	restoreZoom() {
		this.camera.restoreZoom();
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
			toast.error(`placement: ${errorMessage(err)}`),
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
			`${ids.length} ${ids.length === 1 ? 'system' : 'systems'}`,
		);
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
