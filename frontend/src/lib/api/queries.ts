import { queryOptions } from '@tanstack/svelte-query';
import { api } from './client';

// Every read goes through one of these, so the key hierarchy stays coherent in one place.
// Convention: the map socket invalidates ['maps', mapId, ...] prefixes; no page invalidates
// another page's keys except via these prefixes. `key` holds the prefixes writes invalidate;
// `q` holds one queryOptions per api read, named after the api method.

export const key = {
	me: ['me'] as const,
	myCharacters: ['me', 'characters'] as const,
	serverStatus: ['server-status'] as const,
	// The myMaps list; also the prefix over every per-map subtree, so invalidating it
	// refreshes the list and all open map data at once.
	maps: ['maps'] as const,
	map: (id: number) => ['maps', id] as const,
	mapView: (id: number) => ['maps', id, 'view'] as const,
	signatures: (id: number) => ['maps', id, 'signatures'] as const,
	mapCharacters: (id: number) => ['maps', id, 'characters'] as const,
	userSettings: (id: number) => ['maps', id, 'user-settings'] as const,
	access: (id: number) => ['maps', id, 'access'] as const,
	killmails: (id: number) => ['maps', id, 'killmails'] as const,
	history: (id: number) => ['maps', id, 'history'] as const,
	watchlist: (id: number) => ['maps', id, 'watchlist'] as const,
	stale: (id: number) => ['maps', id, 'stale-connections'] as const,
	// Alerts, their events, webhooks and roles share this prefix: the alerts page edits
	// them together, so one invalidation covers all four.
	alerting: (id: number) => ['maps', id, 'alerting'] as const,
	connectionJumps: (id: number, connectionId: number) =>
		['maps', id, 'connection-jumps', connectionId] as const,
};

const FIVE_MINUTES = 5 * 60_000;

export const q = {
	myCharacters: () => queryOptions({ queryKey: key.myCharacters, queryFn: api.myCharacters }),
	myScopes: () => queryOptions({ queryKey: ['me', 'scopes'], queryFn: api.myScopes }),
	myDiscord: () => queryOptions({ queryKey: ['me', 'discord'], queryFn: api.myDiscord }),

	instance: () =>
		queryOptions({ queryKey: ['instance'], queryFn: api.instance, staleTime: Infinity }),
	gridConfig: () =>
		queryOptions({ queryKey: ['grid-config'], queryFn: api.gridConfig, staleTime: Infinity }),
	serverStatus: () =>
		queryOptions({
			queryKey: key.serverStatus,
			queryFn: api.serverStatus,
			refetchInterval: 60_000,
		}),
	// Background refetch stays on for both feeds: a hidden tab must keep the data the
	// jump tracker and the cards read, like the old setInterval did.
	skyhooks: () =>
		queryOptions({
			queryKey: ['skyhooks'],
			queryFn: api.skyhooks,
			refetchInterval: FIVE_MINUTES,
			refetchIntervalInBackground: true,
			staleTime: FIVE_MINUTES,
		}),
	eveScout: () =>
		queryOptions({
			queryKey: ['evescout'],
			queryFn: api.eveScout,
			refetchInterval: FIVE_MINUTES,
			refetchIntervalInBackground: true,
			staleTime: FIVE_MINUTES,
		}),

	myMaps: () => queryOptions({ queryKey: key.maps, queryFn: api.myMaps }),
	mapView: (id: number) =>
		queryOptions({ queryKey: key.mapView(id), queryFn: () => api.fetchMap(id) }),
	listSignatures: (id: number) =>
		queryOptions({ queryKey: key.signatures(id), queryFn: () => api.listSignatures(id) }),
	mapCharacters: (id: number) =>
		queryOptions({ queryKey: key.mapCharacters(id), queryFn: () => api.mapCharacters(id) }),
	mapUserSettings: (id: number) =>
		queryOptions({ queryKey: key.userSettings(id), queryFn: () => api.mapUserSettings(id) }),
	listAccess: (id: number) =>
		queryOptions({ queryKey: key.access(id), queryFn: () => api.listAccess(id) }),
	mapKillmails: (id: number) =>
		queryOptions({ queryKey: key.killmails(id), queryFn: () => api.mapKillmails(id) }),
	mapHistory: (id: number) =>
		queryOptions({ queryKey: key.history(id), queryFn: () => api.mapHistory(id) }),
	listWatchlist: (id: number) =>
		queryOptions({ queryKey: key.watchlist(id), queryFn: () => api.listWatchlist(id) }),
	listStaleConnections: (id: number) =>
		queryOptions({ queryKey: key.stale(id), queryFn: () => api.listStaleConnections(id) }),

	listAlerts: (id: number) =>
		queryOptions({ queryKey: [...key.alerting(id), 'alerts'], queryFn: () => api.listAlerts(id) }),
	alertEvents: (id: number) =>
		queryOptions({ queryKey: [...key.alerting(id), 'events'], queryFn: () => api.alertEvents(id) }),
	listWebhooks: (id: number) =>
		queryOptions({
			queryKey: [...key.alerting(id), 'webhooks'],
			queryFn: () => api.listWebhooks(id),
		}),
	listAlertRoles: (id: number) =>
		queryOptions({
			queryKey: [...key.alerting(id), 'roles'],
			queryFn: () => api.listAlertRoles(id),
		}),

	systemDetails: (id: number, mss: number) =>
		queryOptions({
			queryKey: ['maps', id, 'details', mss],
			queryFn: () => api.systemDetails(id, mss),
		}),
	listConnectionJumps: (id: number, connectionId: number) =>
		queryOptions({
			queryKey: key.connectionJumps(id, connectionId),
			queryFn: () => api.listConnectionJumps(id, connectionId),
		}),
	searchMap: (id: number, term: string) =>
		queryOptions({
			queryKey: ['maps', id, 'search', term],
			queryFn: () => api.searchMap(id, term),
		}),

	searchSystems: (term: string) =>
		queryOptions({ queryKey: ['search', 'systems', term], queryFn: () => api.searchSystems(term) }),
	searchShips: (term: string) =>
		queryOptions({ queryKey: ['search', 'ships', term], queryFn: () => api.searchShips(term) }),
	searchAccessSubjects: (term: string) =>
		queryOptions({
			queryKey: ['search', 'access-subjects', term],
			queryFn: () => api.searchAccessSubjects(term),
		}),

	effectModifiers: (name: string, wormholeClassId: number) =>
		queryOptions({
			queryKey: ['effects', name, wormholeClassId],
			queryFn: () => api.effectModifiers(name, wormholeClassId),
			staleTime: Infinity,
		}),
	threatAnalysis: (solarSystemId: number) =>
		queryOptions({
			queryKey: ['threat', solarSystemId],
			queryFn: () => api.threatAnalysis(solarSystemId),
		}),
	signatureCatalog: () =>
		queryOptions({
			queryKey: ['reference', 'signature-catalog'],
			queryFn: api.signatureCatalog,
			staleTime: Infinity,
			gcTime: Infinity,
		}),
	routingGraph: () =>
		queryOptions({
			queryKey: ['reference', 'routing-graph'],
			queryFn: api.routingGraph,
			staleTime: Infinity,
			gcTime: Infinity,
		}),
};
