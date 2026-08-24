// The characters domain: who is on the map, and this account's own pilots.

import type { CharacterRef } from '$lib/api/types/CharacterRef';
import type { MapCharacter } from '$lib/api/types/MapCharacter';

export interface CharactersHost {
	all(): MapCharacter[];
	mine(): CharacterRef[];
	/** Ask for fresh presence, soon: who is on the map. No-ops signed out. */
	refresh(): void;
	/** Ask for a fresh reading of this account's pilots, soon; the tracker observes it. */
	refreshMine(): void;
}

export class CharactersApi {
	constructor(private host: CharactersHost) {}

	/** Everyone currently placed on the map, whoever they belong to. */
	get all(): MapCharacter[] {
		return this.host.all();
	}

	/** This account's pilots, on the map or not. */
	get mine(): CharacterRef[] {
		return this.host.mine();
	}

	/** Who can receive a waypoint right now. */
	get online(): CharacterRef[] {
		return this.mine.filter((c) => c.online);
	}

	refresh() {
		this.host.refresh();
	}

	refreshMine() {
		this.host.refreshMine();
	}
}
