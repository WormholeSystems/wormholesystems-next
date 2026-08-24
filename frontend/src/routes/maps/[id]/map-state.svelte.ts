// The map page's shared state. Interaction state is owned here, never derived from the
// fetched data, so it survives refetches. The fetched data itself lives in the query
// cache ([`createMapQueries`]); the fields here are views over it, so a refetch can
// never clobber a drag, a selection, or an open menu.

import { untrack } from 'svelte';

import { api, errorMessage } from '$lib/api/client';
import { key } from '$lib/api/queries';
import type { GridConfig } from '$lib/api/types/GridConfig';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
import type { UpdateMapUserSettings } from '$lib/api/types/UpdateMapUserSettings';
import type { MapView } from '$lib/api/types/MapView';
import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
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
import { type MapAction } from './actions';
import { MapCamera } from './map-camera.svelte';
import { createMapQueries, type MapQueries } from './map-queries.svelte';
import { PanelLayoutStore } from './panel-layout.svelte';
import { RoutePlanner } from './route-planner.svelte';
import type { MapEvent } from '$lib/api/types/MapEvent';
import type { MapEventEntry } from '$lib/api/types/MapEventEntry';
import { timeAgo } from '$lib/format';
import { solarSystemId } from '$lib/map/system';
import { atLeast } from '$lib/map/roles';
import { systemResolver } from '$lib/resolve-cache.svelte';

export type { Drag };

/** A placed system in the shape the resolver serves, so both sides answer alike. */
function searchResultOf(placed: Extract<MapSystemView, { kind: 'system' }>): SystemSearchResult {
	return {
		id: placed.solar_system_id,
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
	queries: MapQueries;

	get data() {
		return this.queries.graph.data ?? null;
	}
	get grid() {
		return this.queries.grid.data ?? defaultGrid;
	}
	get sigs() {
		return this.queries.signatures.data ?? [];
	}
	get watchlist() {
		return this.queries.watchlist.data ?? [];
	}
	get eveScout() {
		return this.queries.eveScout.data ?? [];
	}
	get characters() {
		return this.queries.characters.data ?? [];
	}
	get myCharacters() {
		return this.queries.myCharacters.data ?? [];
	}
	get userSettings() {
		return this.queries.settings.data ?? null;
	}

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
	// The history tree plus where the map sits in it, and the live socket state behind
	// the status dot.
	get history() {
		return this.queries.history.data ?? null;
	}
	// Connections critical for over an hour, offered for a one-click sweep.
	get stale() {
		return this.queries.stale.data ?? [];
	}
	socket = $state<SocketState>('connecting');
	// The page holds a loader until both the graph and the arrangement have arrived, so
	// tiles are never painted in the built-in positions and then moved.
	get loaded() {
		return this.queries.graph.data !== undefined;
	}
	// A disabled query stays pending forever, so the signed-out case answers first.
	get settingsLoaded() {
		return !this.signedIn || !this.queries.settings.isPending;
	}
	// Only a first load with nothing to show gates the page; a later failure leaves the
	// view briefly stale instead, because the cache keeps the last good data.
	get loadError() {
		return this.queries.graph.data === undefined && this.queries.graph.isError
			? errorMessage(this.queries.graph.error)
			: '';
	}
	get ready() {
		return this.loaded && this.settingsLoaded;
	}

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
		this.signedIn = signedIn;
		this.queries = createMapQueries(mapId, signedIn, seed, () => this.socket === 'open');
		this.panelLayout = new PanelLayoutStore({
			hiddenPanels: () => this.userSettings?.hidden_panels ?? null,
			setHiddenPanels: (panels) =>
				this.queries.patchSettingsLocal((s) => ({ ...s, hidden_panels: panels })),
			save: (layouts, hidden) => {
				const write = this.patchUserSettings({
					layout_breakpoints: layouts,
					hidden_panels: hidden,
				}).then((saved) => saved.layout_breakpoints ?? null);
				this.run('saveLayout', write);
				return write;
			},
		});
		this.route = new RoutePlanner(this, this.queries.client);
		// Seeded from the page's load, so the first frame already has the arrangement; a
		// map opened without a seed gets it once the settings query lands.
		let layoutsSeeded = seed.settings != null;
		if (seed.settings) this.panelLayout.seed(seed.settings.layout_breakpoints ?? null);
		$effect(() => {
			const settings = this.queries.settings.data;
			if (!settings || layoutsSeeded) return;
			layoutsSeeded = true;
			this.panelLayout.seed(settings.layout_breakpoints ?? null);
		});

		// Each graph payload's arrival: the resolver learns the placed systems, and a
		// pending move is dropped once the server position matches it (ours landed) or
		// the system is gone.
		$effect(() => {
			const data = this.queries.graph.data;
			if (!data) return;
			this.shareResolved();
			const pending = { ...untrack(() => this.pending) };
			let changed = false;
			for (const [id, p] of Object.entries(pending)) {
				const s = data.systems.find((s) => s.id === Number(id));
				if (!s || (Math.abs(s.position_x - p.x) <= 0.5 && Math.abs(s.position_y - p.y) <= 0.5)) {
					delete pending[Number(id)];
					changed = true;
				}
			}
			if (changed) this.pending = pending;
		});
	}

	/** Panels there is nobody to fill in: both of these are about the account, not the map. */
	unavailablePanels = $derived(new Set<PanelId>(this.signedIn ? [] : ['characters', 'skyhooks']));

	/** Ask for fresh presence, soon: who is on the map, and where this account's pilots are. */
	refreshCharacters() {
		if (!this.signedIn) return;
		void this.queries.client.invalidateQueries({ queryKey: key.mapCharacters(this.mapId) });
	}

	/**
	 * A system in the shape every picker and the context menu expect, whether or not it is
	 * on the map. Returns null until [`ensureResolved`] has fetched an off-map one.
	 */
	systemInfo(id: number): SystemSearchResult | null {
		const placed = this.systems.find((s) => solarSystemId(s) === id);
		if (placed?.kind === 'system') return searchResultOf(placed);
		return systemResolver.get(id) ?? null;
	}

	/** Fetch display data for any of `ids` that is neither on the map nor already known. */
	ensureResolved(ids: number[]) {
		systemResolver.ensure(ids);
	}

	/** Placed systems arrive with the graph; the resolver learns them so no panel refetches one. */
	private shareResolved() {
		systemResolver.seed(
			this.systems.flatMap((s) => (s.kind === 'system' ? [searchResultOf(s)] : [])),
		);
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

	cleanStale() {
		this.run('cleanStale', api.cleanStaleConnections({ map_id: this.mapId }));
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

	/** Refetch what one socket frame invalidated; the table lives in [`keysFor`]. */
	applyEvent(event: MapEvent | null) {
		this.queries.applyEvent(event);
	}

	/** Refetch the whole map subtree; resolves once the active refetches settle. */
	refetch() {
		return this.queries.invalidateAll();
	}

	/**
	 * Run an action and say how it went. What it says lives in [`MAP_ACTIONS`] rather than
	 * at the call site; the refetch policy lives on the mutation, in [`createMapQueries`].
	 */
	run(action: MapAction, promise: Promise<unknown>, detail?: string) {
		this.queries.write.mutate({ action, exec: () => promise, detail });
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
		this.queries.patchSettingsLocal((s) => ({ ...s, layout_override: own ?? undefined }));
		this.patchUserSettings({ layout_override: own }).catch((err) =>
			toast.error(`placement: ${errorMessage(err)}`),
		);
	}

	/**
	 * Write one or more of the viewer's own settings. Everything goes through the one
	 * mutation, which also holds the write against a slower read already on the wire.
	 */
	patchUserSettings(patch: UpdateMapUserSettings): Promise<MapUserSettings> {
		return this.queries.saveSettings.mutateAsync(patch);
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
