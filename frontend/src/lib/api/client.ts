// Typed client for the Axum JSON API. Browser-side calls use relative paths (the vite
// dev proxy / Caddy route them); server-side loads go through `$lib/server/api` instead.

import type { AddConnection } from './types/AddConnection';
import type { AddConnectionJump } from './types/AddConnectionJump';
import type { AddSignature } from './types/AddSignature';
import type { AddSystem } from './types/AddSystem';
import type { CharacterRef } from './types/CharacterRef';
import type { ConnectionJump } from './types/ConnectionJump';
import type { CharacterStatus } from './types/CharacterStatus';
import type { CharacterSummary } from './types/CharacterSummary';
import type { ClearMap } from './types/ClearMap';
import type { EffectModifier } from './types/EffectModifier';
import type { EveScoutEdge } from './types/EveScoutEdge';
import type { GridConfig } from './types/GridConfig';
import type { LinkSignature } from './types/LinkSignature';
import type { Map } from './types/Map';
import type { MapCharacter } from './types/MapCharacter';
import type { MapConnection } from './types/MapConnection';
import type { MapEntry } from './types/MapEntry';
import type { MapSolarSystem } from './types/MapSolarSystem';
import type { MapUserSettings } from './types/MapUserSettings';
import type { MapView } from './types/MapView';
import type { MoveSystems } from './types/MoveSystems';
import type { PasteSignatures } from './types/PasteSignatures';
import type { RemoveConnection } from './types/RemoveConnection';
import type { RemoveConnectionJump } from './types/RemoveConnectionJump';
import type { RemoveSignature } from './types/RemoveSignature';
import type { RemoveSignatures } from './types/RemoveSignatures';
import type { RemoveSystems } from './types/RemoveSystems';
import type { SetAlias } from './types/SetAlias';
import type { SetConnectionStatus } from './types/SetConnectionStatus';
import type { SetHome } from './types/SetHome';
import type { SetNotes } from './types/SetNotes';
import type { SetOccupier } from './types/SetOccupier';
import type { SetPinned } from './types/SetPinned';
import type { SetRally } from './types/SetRally';
import type { SetStatus } from './types/SetStatus';
import type { SetWaypointAllBody } from './types/SetWaypointAllBody';
import type { SetWaypointBody } from './types/SetWaypointBody';
import type { Signature } from './types/Signature';
import type { ShipSearchResult } from './types/ShipSearchResult';
import type { SignatureCatalog } from './types/SignatureCatalog';
import type { AddWatchlistEntry } from './types/AddWatchlistEntry';
import type { RemoveWatchlistEntry } from './types/RemoveWatchlistEntry';
import type { SetWatchlistPinned } from './types/SetWatchlistPinned';
import type { SystemDetails } from './types/SystemDetails';
import type { AccessEntry } from './types/AccessEntry';
import type { StaleConnection } from './types/StaleConnection';
import type { CleanStaleConnections } from './types/CleanStaleConnections';
import type { AccessSubject } from './types/AccessSubject';
import type { SetAccess } from './types/SetAccess';
import type { RevokeAccess } from './types/RevokeAccess';
import type { UpdateMap } from './types/UpdateMap';
import type { MapEventEntry } from './types/MapEventEntry';
import type { UndoMapEvent } from './types/UndoMapEvent';
import type { WatchlistEntry } from './types/WatchlistEntry';
import type { ThreatAnalysis } from './types/ThreatAnalysis';
import type { SystemSearchResult } from './types/SystemSearchResult';
import type { UnlinkSignature } from './types/UnlinkSignature';
import type { UpdateConnectionJump } from './types/UpdateConnectionJump';
import type { UpdateSignature } from './types/UpdateSignature';
import type { UpdateMapUserSettings } from './types/UpdateMapUserSettings';

export class ApiError extends Error {
	status: number;

	constructor(status: number, message: string) {
		super(message);
		this.status = status;
	}
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
	const res = await fetch(path, init);
	if (!res.ok) {
		let message = res.statusText;
		try {
			const body = await res.json();
			if (typeof body?.error === 'string') message = body.error;
		} catch {
			// non-JSON error body; keep the status text
		}
		throw new ApiError(res.status, message);
	}
	return res.json();
}

function get<T>(path: string): Promise<T> {
	return request<T>(path);
}

function post<T>(path: string, body: unknown): Promise<T> {
	return request<T>(path, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

function del<T>(path: string): Promise<T> {
	return request<T>(path, { method: 'DELETE' });
}

export const api = {
	// Auth / identity
	me: () => get<CharacterSummary | null>('/api/me'),
	meStatus: () => get<CharacterStatus | null>('/api/me/status'),
	myCharacters: () => get<CharacterRef[]>('/api/me/characters'),
	switchCharacter: (characterId: number) =>
		post<null>('/api/me/switch-character', { character_id: characterId }),
	removeCharacter: (characterId: number) =>
		post<null>('/api/me/remove-character', { character_id: characterId }),

	// ESI waypoints
	setWaypoint: (body: SetWaypointBody) => post<null>('/api/waypoints', body),
	setWaypointAll: (body: SetWaypointAllBody) => post<null>('/api/waypoints/all', body),

	// Config / reference data
	gridConfig: () => get<GridConfig>('/api/grid-config'),
	effectModifiers: (name: string, wormholeClassId: number) =>
		get<EffectModifier[]>(
			`/api/effects?name=${encodeURIComponent(name)}&class=${wormholeClassId}`
		),
	searchSystems: (query: string) =>
		get<SystemSearchResult[]>(`/api/systems/search?q=${encodeURIComponent(query)}`),
	resolveSystems: (ids: number[]) =>
		get<SystemSearchResult[]>(`/api/systems/resolve?ids=${ids.join(',')}`),
	threatAnalysis: (solarSystemId: number) =>
		get<ThreatAnalysis>(`/api/threat/${solarSystemId}`),
	// The response is cached for a day; the version param busts browser caches when
	// the payload shape grows.
	routingGraph: () =>
		get<{
			adjacency: Record<string, number[]>;
			security: Record<string, number>;
			jove: number[];
			stations: number[];
			services: {
				id: number;
				name: string;
				stations: { id: number; name: string; solar_system_id: number }[];
			}[];
		}>('/api/routing-graph?v=3'),

	// Maps
	myMaps: () => get<MapEntry[]>('/api/maps'),
	createMap: (name: string) => post<Map>('/api/maps', { name }),
	deleteMap: (mapId: number) => del<null>(`/api/maps/${mapId}`),
	fetchMap: (mapId: number) => get<MapView>(`/api/maps/${mapId}`),
	listSignatures: (mapId: number) => get<Signature[]>(`/api/maps/${mapId}/signatures`),
	mapCharacters: (mapId: number) => get<MapCharacter[]>(`/api/maps/${mapId}/characters`),
	mapUserSettings: (mapId: number) => get<MapUserSettings>(`/api/maps/${mapId}/settings/user`),
	updateMapUserSettings: (mapId: number, update: UpdateMapUserSettings) =>
		post<MapUserSettings>(`/api/maps/${mapId}/settings/user`, update),

	// Systems
	addSystem: (cmd: AddSystem) =>
		post<MapSolarSystem>(`/api/maps/${cmd.map_id}/systems/add`, cmd),
	moveSystems: (cmd: MoveSystems) => post<null>(`/api/maps/${cmd.map_id}/systems/move`, cmd),
	removeSystems: (cmd: RemoveSystems) =>
		post<null>(`/api/maps/${cmd.map_id}/systems/remove`, cmd),
	clearMap: (cmd: ClearMap) => post<null>(`/api/maps/${cmd.map_id}/clear`, cmd),
	setAlias: (cmd: SetAlias) => post<null>(`/api/maps/${cmd.map_id}/systems/set-alias`, cmd),
	setStatus: (cmd: SetStatus) => post<null>(`/api/maps/${cmd.map_id}/systems/set-status`, cmd),
	setOccupier: (cmd: SetOccupier) =>
		post<null>(`/api/maps/${cmd.map_id}/systems/set-occupier`, cmd),
	setHome: (cmd: SetHome) => post<null>(`/api/maps/${cmd.map_id}/systems/set-home`, cmd),
	setNotes: (cmd: SetNotes) => post<null>(`/api/maps/${cmd.map_id}/systems/set-notes`, cmd),
	systemDetails: (mapId: number, mss: number) =>
		get<SystemDetails>(`/api/maps/${mapId}/systems/${mss}/details`),
	setRally: (cmd: SetRally) => post<null>(`/api/maps/${cmd.map_id}/systems/set-rally`, cmd),
	setPinned: (cmd: SetPinned) => post<null>(`/api/maps/${cmd.map_id}/systems/set-pinned`, cmd),

	// Connections
	addConnection: (cmd: AddConnection) =>
		post<MapConnection>(`/api/maps/${cmd.map_id}/connections/add`, cmd),
	setConnectionStatus: (cmd: SetConnectionStatus) =>
		post<MapConnection>(`/api/maps/${cmd.map_id}/connections/set-status`, cmd),
	removeConnection: (cmd: RemoveConnection) =>
		post<null>(`/api/maps/${cmd.map_id}/connections/remove`, cmd),
	listConnectionJumps: (mapId: number, connectionId: number) =>
		get<ConnectionJump[]>(`/api/maps/${mapId}/connections/${connectionId}/jumps`),
	addConnectionJump: (cmd: AddConnectionJump) =>
		post<ConnectionJump>(`/api/maps/${cmd.map_id}/connections/jumps/add`, cmd),
	updateConnectionJump: (cmd: UpdateConnectionJump) =>
		post<ConnectionJump>(`/api/maps/${cmd.map_id}/connections/jumps/update`, cmd),
	removeConnectionJump: (cmd: RemoveConnectionJump) =>
		post<null>(`/api/maps/${cmd.map_id}/connections/jumps/remove`, cmd),
	searchShips: (query: string) =>
		get<ShipSearchResult[]>(`/api/ships/search?q=${encodeURIComponent(query)}`),

	// Navigation
	eveScout: () => get<EveScoutEdge[]>('/api/evescout'),
	listWatchlist: (mapId: number) => get<WatchlistEntry[]>(`/api/maps/${mapId}/watchlist`),
	addWatchlistEntry: (cmd: AddWatchlistEntry) =>
		post<WatchlistEntry>(`/api/maps/${cmd.map_id}/watchlist/add`, cmd),
	setWatchlistPinned: (cmd: SetWatchlistPinned) =>
		post<WatchlistEntry>(`/api/maps/${cmd.map_id}/watchlist/set-pinned`, cmd),
	removeWatchlistEntry: (cmd: RemoveWatchlistEntry) =>
		post<null>(`/api/maps/${cmd.map_id}/watchlist/remove`, cmd),

	// Access / settings
	updateMap: (cmd: UpdateMap) => post<Map>(`/api/maps/${cmd.map_id}/update`, cmd),
	listAccess: (mapId: number) => get<AccessEntry[]>(`/api/maps/${mapId}/access`),
	searchAccessSubjects: (query: string) =>
		get<AccessSubject[]>(`/api/access-subjects/search?q=${encodeURIComponent(query)}`),
	setAccess: (cmd: SetAccess) => post<null>(`/api/maps/${cmd.map_id}/access/set`, cmd),
	revokeAccess: (cmd: RevokeAccess) => post<null>(`/api/maps/${cmd.map_id}/access/revoke`, cmd),

	listStaleConnections: (mapId: number) =>
		get<StaleConnection[]>(`/api/maps/${mapId}/connections/stale`),
	cleanStaleConnections: (cmd: CleanStaleConnections) =>
		post<number>(`/api/maps/${cmd.map_id}/connections/clean-stale`, cmd),

	// History
	listMapEvents: (mapId: number) => get<MapEventEntry[]>(`/api/maps/${mapId}/events`),
	undoMapEvent: (cmd: UndoMapEvent) => post<null>(`/api/maps/${cmd.map_id}/events/undo`, cmd),

	// Signatures
	signatureCatalog: () => get<SignatureCatalog>('/api/signature-types'),
	addSignature: (cmd: AddSignature) =>
		post<Signature>(`/api/maps/${cmd.map_id}/signatures/add`, cmd),
	updateSignature: (cmd: UpdateSignature) =>
		post<Signature>(`/api/maps/${cmd.map_id}/signatures/update`, cmd),
	removeSignaturesBulk: (cmd: RemoveSignatures) =>
		post<null>(`/api/maps/${cmd.map_id}/signatures/remove-bulk`, cmd),
	pasteSignatures: (cmd: PasteSignatures) =>
		post<null>(`/api/maps/${cmd.map_id}/signatures/paste`, cmd),
	linkSignature: (cmd: LinkSignature) =>
		post<Signature>(`/api/maps/${cmd.map_id}/signatures/link`, cmd),
	unlinkSignature: (cmd: UnlinkSignature) =>
		post<Signature>(`/api/maps/${cmd.map_id}/signatures/unlink`, cmd),
	removeSignature: (cmd: RemoveSignature) =>
		post<null>(`/api/maps/${cmd.map_id}/signatures/remove`, cmd)
};
