// The map page's shared state. Interaction state is owned here, never derived from the
// fetched data, so it survives refetches. The fetched data itself lives in the query
// cache ([`createMapQueries`]); the fields here are views over it, so a refetch can
// never clobber a drag, a selection, or an open menu.

import { untrack } from 'svelte';

import { api, errorMessage } from '$lib/api/client';
import { key, q } from '$lib/api/queries';
import type { GridConfig } from '$lib/api/types/GridConfig';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
import type { UpdateMapUserSettings } from '$lib/api/types/UpdateMapUserSettings';
import type { MapView } from '$lib/api/types/MapView';
import type { SystemDetails } from '$lib/api/types/SystemDetails';
import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
import type { SocketState } from '$lib/ws';
import type { PanelId } from './panels/registry';
import type { RoutingSettings } from '$lib/routing/algorithm';
import { NODE_W, clamp, heuristicSize } from '$lib/map/helpers';
import { freeEdges, treeEdges, type EdgeGeometry } from '$lib/map/edges';
import { draggedPositions, type Drag } from '$lib/map/gestures';
import type { Vec2 } from '$lib/map/helpers';
import { compareForTree, computeTreeLayout } from '$lib/map/tree';
import { routeOrigin } from '$lib/routing/origin';
import { ghostSignatureIds } from '$lib/map/ghosts';
import { layoutMode } from '$lib/map/layout-mode';
import { orphanedSystems } from '$lib/map/orphans';
import { toast } from 'svelte-sonner';
import { type MapAction } from '$lib/map/actions';
import { MapCamera } from './map-camera.svelte';
import { HistoryStore } from './history.svelte';
import { createMapQueries, type MapQueries } from './map-queries.svelte';
import { PanelLayoutStore } from './panel-layout.svelte';
import { RoutePlanner, type RouteHost } from './route-planner.svelte';
import type { TrackerHost } from './tracking.svelte';
import type { MapEvent } from '$lib/api/types/MapEvent';
import { loadCatalog } from '$lib/map/signatures';
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
	panels: PanelLayoutStore;
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
	// The history tree plus where the map sits in it (`history.data`), and the cursor
	// controls over it.
	history: HistoryStore;
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
	routeOrigin = $derived.by(() =>
		routeOrigin(
			this.route.fromId,
			this.activeSystem ? solarSystemId(this.activeSystem) : null,
			this.myCharacters,
		),
	);

	routingSettings = $derived<RoutingSettings>({
		preference: this.userSettings?.route_preference ?? 'shorter',
		securityPenalty: this.userSettings?.security_penalty ?? 50,
		allowTimeStatus: this.userSettings?.route_allow_time_status ?? 'critical',
		allowMassStatus: this.userSettings?.route_allow_mass_status ?? 'reduced',
	});
	useEveScout = $derived(this.userSettings?.route_use_evescout ?? false);

	connections = $derived(this.data?.connections ?? []);
	nodeH = $derived(2 * this.grid.cell_size);

	layout = $derived.by(() =>
		layoutMode(this.data?.map ?? null, this.userSettings?.layout_override),
	);

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

	ghostSignatures = $derived.by(() => ghostSignatureIds(this.systems, this.connections, this.sigs));

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
		this.history = new HistoryStore({
			mapId,
			data: () => this.queries.history.data ?? null,
			run: (action: MapAction, promise: Promise<unknown>, detail?: string) =>
				this.run(action, promise, detail),
		});
		this.panels = new PanelLayoutStore({
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
		this.route = new RoutePlanner(this.routeHost());
		// Seeded from the page's load, so the first frame already has the arrangement; a
		// map opened without a seed gets it once the settings query lands.
		let layoutsSeeded = seed.settings != null;
		if (seed.settings) this.panels.seed(seed.settings.layout_breakpoints ?? null);
		$effect(() => {
			const settings = this.queries.settings.data;
			if (!settings || layoutsSeeded) return;
			layoutsSeeded = true;
			this.panels.seed(settings.layout_breakpoints ?? null);
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

	/** Move placements to where a drag dropped them; the optimistic override is the caller's. */
	moveSystems(moves: { map_solar_system_id: number; x: number; y: number }[]) {
		this.run('moveSystems', api.moveSystems({ map_id: this.mapId, moves }));
	}

	/** Join two placements with a wormhole, sized by what the two systems suggest. */
	connectSystems(from: number, to: number) {
		this.run(
			'addConnection',
			api.addConnection({
				map_id: this.mapId,
				from_system: from,
				to_system: to,
				kind: 'wormhole',
				size: heuristicSize(this.systems, from, to),
			}),
		);
	}

	/** The signature type catalog, cached forever in the query client. */
	loadCatalog() {
		return loadCatalog(this.queries.client);
	}

	/** Ask for a fresh jump log for one connection; a closed popover just goes stale. */
	refreshConnectionJumps(connectionId: number) {
		void this.queries.client.invalidateQueries({
			queryKey: key.connectionJumps(this.mapId, connectionId),
		});
	}

	/** The local echo for a saved note, so it reads back before the server confirms it. */
	setSystemNotesLocal(mapSolarSystemId: number, notes: string | null) {
		this.queries.client.setQueryData(
			q.systemDetails(this.mapId, mapSolarSystemId).queryKey,
			(d: SystemDetails | undefined) => d && { ...d, notes },
		);
	}

	/** An optimistic local edit of the viewer's settings, without a round trip. */
	patchSettingsLocal(update: (s: MapUserSettings) => MapUserSettings) {
		this.queries.patchSettingsLocal(update);
	}

	/** Ask for a fresh reading of this account's pilots, soon; the tracker observes it. */
	refreshMyCharacters() {
		if (!this.signedIn) return;
		void this.queries.client.invalidateQueries({ queryKey: key.myCharacters });
	}

	/** The jump tracker's narrow view of this map, commands included. */
	trackerHost(): TrackerHost {
		return {
			myCharacters: () => this.myCharacters,
			systems: () => this.systems,
			connections: () => this.connections,
			sigs: () => this.sigs,
			grid: () => this.grid,
			settings: () => this.userSettings,
			naming: () => this.data?.map.naming ?? null,
			stargates: () => this.route.stargates,
			whenRoutingLoaded: () => this.route.whenLoaded(),
			loadCatalog: () => loadCatalog(this.queries.client),
			resolveSystem: (id) => systemResolver.resolve(id),
			trackJump: (cmd) => this.run('trackJump', api.trackJump({ ...cmd, map_id: this.mapId })),
			resolveGhost: (cmd) =>
				this.run('assignSystem', api.resolveGhostSystem({ ...cmd, map_id: this.mapId })),
		};
	}

	/** The planner's narrow view of this map; tables come from the query cache. */
	private routeHost(): RouteHost {
		return {
			mapId: this.mapId,
			systems: () => this.systems,
			connections: () => this.connections,
			sigs: () => this.sigs,
			eveScout: () => this.eveScout,
			useEveScout: () => this.useEveScout,
			loadTables: () => this.queries.client.ensureQueryData(q.routingGraph()),
		};
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

	cleanStale() {
		this.run('cleanStale', api.cleanStaleConnections({ map_id: this.mapId }));
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
