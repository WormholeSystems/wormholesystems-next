// Building the chain as you fly it.
//
// Watching the active character's system is the whole trick, and every guard here exists
// because some other way of moving between systems also changes that number: switching
// character, logging in, taking a gate. Only a real jump through an unmapped hole should
// put anything on the map, so anything ambiguous is dropped rather than guessed at.

import { api } from '$lib/api/client';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { Signature } from '$lib/api/types/Signature';
import type { SignatureCatalog } from '$lib/api/types/SignatureCatalog';
import type { SignatureTypeInfo } from '$lib/api/types/SignatureTypeInfo';
import { aliasTargetKind, suggestAlias, type AliasScheme } from '$lib/alias';
import { freePosition } from '$lib/map/helpers';
import { classMeta, isWormholeClass } from '$lib/map/classes';
import { loadCatalog, typeById } from '$lib/map/signatures';
import { groupSignatures, type SignatureGroups } from '$lib/signatures/compatibility';
import type { MapState } from './map-state.svelte';

/** A jump waiting on the user to say which signature it was. */
export interface JumpPrompt {
	origin: MapSystemView;
	targetSolarSystemId: number;
	targetName: string;
	targetClassId: number | null;
	targetSecurity: number;
	/** The placement the target already has on the map, if it was reached another way. */
	existing: MapSystemView | null;
	groups: SignatureGroups;
	catalog: SignatureCatalog;
	suggestedAlias: string | null;
	/** Where the new system would go, decided when the jump happened. */
	at: { x: number; y: number };
}

let gatesPromise: Promise<Map<number, number[]>> | null = null;

/** Stargate adjacency, fetched once per session (the response is cached for a day). */
function loadGates(): Promise<Map<number, number[]>> {
	gatesPromise ??= api
		.routingGraph()
		.then(
			(g) => new Map(Object.entries(g.adjacency).map(([k, v]) => [Number(k), v as number[]]))
		)
		.catch(() => new Map<number, number[]>());
	return gatesPromise;
}

/** The mass and lifetime a hole starts at, taken from the signature if it was scanned. */
function statusFromSignature(signature: Signature | null) {
	return {
		size: signature?.size ?? null,
		mass_status: signature?.mass_status ?? null,
		time_status: signature?.time_status ?? null
	};
}

/** The size a wormhole type promises, which beats whatever was typed on the signature. */
function sizeFromType(type: SignatureTypeInfo | null): Signature['size'] {
	const jump = type?.max_jump_mass;
	if (!jump) return null;
	if (jump <= 5_000_000) return 'small';
	if (jump <= 300_000_000) return 'medium';
	if (jump <= 1_000_000_000) return 'large';
	return 'xl';
}

export class JumpTracker {
	private map: MapState;

	/** The open prompt, or null. The dialog renders this and nothing else. */
	prompt = $state<JumpPrompt | null>(null);

	// What the last poll saw, so a change can be told apart from the first reading.
	private seenCharacterId: number | null = null;
	private seenSystemId: number | null = null;

	// Refreshes are serialised rather than overlapped: two in flight can come back out of
	// order, and an older reply landing after a newer one reads as a jump in the wrong
	// direction. A trigger arriving mid-flight is coalesced into one follow-up run.
	private refreshing: Promise<void> | null = null;
	private pending = false;

	constructor(map: MapState) {
		this.map = map;
	}

	private get enabled(): boolean {
		return this.map.userSettings?.tracking_allowed === true;
	}

	/** Reload where the pilot is and take a reading. Safe to call from several triggers. */
	refresh(): Promise<void> {
		this.pending = true;
		if (this.refreshing) return this.refreshing;
		this.refreshing = (async () => {
			while (this.pending) {
				this.pending = false;
				await this.map.loadMyCharacters();
				this.observe();
			}
		})().finally(() => (this.refreshing = null));
		return this.refreshing;
	}

	/**
	 * Take a reading of where the active character is.
	 *
	 * Called on every character refresh. Returns without doing anything unless two
	 * consecutive readings show the same character in two different systems, which is the
	 * only shape a jump can have.
	 */
	observe() {
		const active = this.map.myCharacters.find((c) => c.is_active) ?? null;
		const characterId = active?.character_id ?? null;
		const systemId = active?.online ? (active.solar_system_id ?? null) : null;

		const fromCharacter = this.seenCharacterId;
		const fromSystem = this.seenSystemId;
		this.seenCharacterId = characterId;
		this.seenSystemId = systemId;

		if (!this.enabled) return;
		// A missing reading on either side is a login, a logout, or the first poll of the
		// session. None of them is a jump, and the last known system may be hours stale.
		if (fromSystem === null || systemId === null) return;
		if (fromSystem === systemId) return;
		// Switching character moves the watched system id without anyone flying anywhere;
		// acting on it would connect two pilots' systems.
		if (characterId !== fromCharacter) return;

		this.handleJump(fromSystem, systemId);
	}

	/**
	 * The unmapped holes already drawn off this system, by the connection that draws them.
	 *
	 * Flying one of these is the moment it stops being a ghost, so the jump resolves the
	 * node that is already there instead of putting the same system on the map twice.
	 */
	private ghostsFrom(origin: MapSystemView): Map<number, number> {
		const ghosts = new Map<number, number>();
		for (const c of this.map.connections) {
			const other =
				c.from_system === origin.id
					? c.to_system
					: c.to_system === origin.id
						? c.from_system
						: null;
			if (other === null) continue;
			const placement = this.map.systems.find((s) => s.id === other);
			if (placement && placement.solar_system_id === null) ghosts.set(c.id, other);
		}
		return ghosts;
	}

	private async handleJump(fromSystemId: number, toSystemId: number) {
		const map = this.map;
		// Only a jump *out of* the mapped chain extends it. Flying around known space with
		// the map open should not start drawing it.
		const origin = map.systems.find((s) => s.solar_system_id === fromSystemId) ?? null;
		if (!origin) return;

		const gates = await loadGates();
		if (gates.get(fromSystemId)?.includes(toSystemId)) return;

		const existing = map.systems.find((s) => s.solar_system_id === toSystemId) ?? null;
		const linked = existing ? this.existingConnection(origin, existing) : null;
		// Already mapped and already explained: there is nothing left to record.
		if (linked?.signature) return;

		const catalog = await loadCatalog();
		const originSignatures = map.sigs.filter((s) => s.solar_system_id === fromSystemId);
		const target = await this.describeTarget(toSystemId, existing);
		if (!target) return;

		const ghosts = this.ghostsFrom(origin);
		const groups = groupSignatures(
			originSignatures,
			new Map(catalog.types.map((t) => [t.id, t])),
			target.classId,
			new Set(ghosts.keys())
		);

		// Nothing to ask about: either the user turned the question off, or the origin has
		// no signature that could be this hole. A connection with no signature to link is
		// all the server would accept anyway.
		if (!map.userSettings?.prompt_for_signature || groups.likely.length === 0) {
			if (linked) return;
			// Nobody to ask, but the chain already says there is exactly one unflown hole
			// here: that is the one just flown.
			if (ghosts.size === 1) {
				const [ghost] = ghosts.values();
				this.resolve(ghost, toSystemId);
				return;
			}
			this.submit({
				origin,
				targetSolarSystemId: toSystemId,
				signaturePk: null,
				alias: this.suggestAliasFor(origin, target, existing),
				at: this.placeNear(origin)
			});
			return;
		}

		this.prompt = {
			origin,
			targetSolarSystemId: toSystemId,
			targetName: target.name,
			targetClassId: target.classId,
			targetSecurity: target.security,
			existing,
			groups,
			catalog,
			suggestedAlias: this.suggestAliasFor(origin, target, existing),
			at: this.placeNear(origin)
		};
	}

	/** The connection already joining two placements, and the signature explaining it. */
	private existingConnection(origin: MapSystemView, target: MapSystemView) {
		const connection = this.map.connections.find(
			(c) =>
				(c.from_system === origin.id && c.to_system === target.id) ||
				(c.from_system === target.id && c.to_system === origin.id)
		);
		if (!connection) return null;
		return {
			connection,
			signature: this.map.sigs.find((s) => s.connection_id === connection.id) ?? null
		};
	}

	/**
	 * What we know about where the character ended up. A system already on the map carries
	 * its own class; anywhere else has to be looked up.
	 */
	private async describeTarget(solarSystemId: number, existing: MapSystemView | null) {
		if (existing?.name != null) {
			return {
				name: existing.name,
				classId: existing.wormhole_class_id,
				security: existing.security_status ?? 0
			};
		}
		try {
			const [hit] = await api.resolveSystems([solarSystemId]);
			if (!hit) return null;
			return { name: hit.name, classId: hit.wormhole_class_id, security: hit.security };
		} catch {
			return null;
		}
	}

	private suggestAliasFor(
		origin: MapSystemView,
		target: { classId: number | null; security: number },
		existing: MapSystemView | null
	): string | null {
		// A system already on the map keeps the name the chain knows it by.
		if (existing?.alias) return existing.alias;
		if (!this.map.userSettings?.suggest_alias) return null;

		const naming = this.map.data?.map.naming;
		const targetIsWormhole = isWormholeClass(target.classId);
		return suggestAlias({
			parentAlias: origin.alias,
			targetIsWormhole,
			originIsWormhole: isWormholeClass(origin.wormhole_class_id),
			aliases: this.map.systems
				.map((s) => s.alias)
				.filter((alias): alias is string => Boolean(alias)),
			scheme: naming?.alias_scheme as AliasScheme | undefined,
			targetKind: aliasTargetKind(
				targetIsWormhole,
				classMeta(target.classId, target.security).short
			),
			ignoredAlias: naming?.ignored_alias
		});
	}

	/** The first free slot beside the system jumped from. */
	private placeNear(origin: MapSystemView): { x: number; y: number } {
		return freePosition(
			this.map.systems,
			{ x: origin.position_x, y: origin.position_y },
			this.map.grid
		);
	}

	/** Post the jump. One command, so a mis-picked signature is one undo. */
	submit(choice: {
		origin: MapSystemView;
		targetSolarSystemId: number;
		signaturePk: number | null;
		alias: string | null;
		at: { x: number; y: number };
		size?: Signature['size'];
		massStatus?: Signature['mass_status'];
		timeStatus?: Signature['time_status'];
	}) {
		const signature =
			choice.signaturePk === null
				? null
				: (this.map.sigs.find((s) => s.id === choice.signaturePk) ?? null);

		// That signature is already drawn as an unflown hole: this jump says where it goes.
		if (signature?.connection_id != null) {
			const ghost = this.ghostsFrom(choice.origin).get(signature.connection_id);
			if (ghost !== undefined) {
				this.resolve(ghost, choice.targetSolarSystemId);
				return;
			}
		}

		const fromSignature = statusFromSignature(signature);
		const catalog = this.prompt?.catalog ?? null;
		const type =
			signature && catalog ? typeById(catalog, signature.signature_type_id) : null;

		this.map.run(
			'track jump',
			api.trackJump({
				map_id: this.map.mapId,
				from_map_solar_system_id: choice.origin.id,
				to_solar_system_id: choice.targetSolarSystemId,
				x: choice.at.x,
				y: choice.at.y,
				// The optional fields are omitted rather than nulled: absent means "nothing
				// known", which is what an unscanned hole actually is.
				signature_pk: choice.signaturePk ?? undefined,
				alias: choice.alias?.trim() || undefined,
				size: choice.size ?? sizeFromType(type) ?? fromSignature.size ?? undefined,
				mass_status: choice.massStatus ?? fromSignature.mass_status ?? undefined,
				time_status: choice.timeStatus ?? fromSignature.time_status ?? undefined
			})
		);
	}

	/** The ghost turned out to be a real system: name it where it already sits. */
	private resolve(ghostPlacementId: number, solarSystemId: number) {
		this.map.run(
			'assign system',
			api.resolveGhostSystem({
				map_id: this.map.mapId,
				map_solar_system_id: ghostPlacementId,
				solar_system_id: solarSystemId
			})
		);
	}

	dismiss() {
		this.prompt = null;
	}
}
