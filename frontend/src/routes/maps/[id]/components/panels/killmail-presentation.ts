// Reading a killmail for the card: who died, who did it, and how loudly to say it.

import type { MapKillmail } from '$lib/api/types/MapKillmail';
import type { MapSystemView } from '$lib/api/types/MapSystemView';
import type { IskSeverity } from '$lib/format';
import { solarSystemId } from '$lib/map/system';

export const ISK_TONE: Record<IskSeverity, string> = {
	unknown: 'text-muted-foreground/60',
	routine: 'text-muted-foreground',
	notable: 'font-semibold text-amber-400',
	severe: 'font-semibold text-red-400',
};

/** An NPC kill in the chain is noise; a solo kill is a hunter. */
export function crowdTone(kill: MapKillmail): string {
	if (kill.is_npc) return 'text-muted-foreground/50';
	if (kill.is_solo) return 'text-amber-400';
	return 'text-muted-foreground';
}

export function crowdLabel(kill: MapKillmail): string {
	if (kill.is_npc) return 'Killed by NPCs';
	if (kill.is_solo) return 'Solo kill';
	return `${kill.attacker_count} attackers`;
}

export function partyName(party: MapKillmail['victim']): string {
	const ticker = party.alliance_ticker ?? party.corporation_ticker;
	const who = party.character_name ?? 'Unknown pilot';
	return ticker ? `${who} [${ticker}]` : who;
}

/** Who they fly for, spelled out. The row has room for a ticker at most. */
export function partyOrg(party: MapKillmail['victim']): string | null {
	const corp = party.corporation_name;
	const alliance = party.alliance_name;
	if (corp && alliance) return `${corp} · ${alliance}`;
	return alliance ?? corp ?? null;
}

/**
 * The placed systems as a stable value for a query key: sorted and joined, so the
 * wholesale array replacement every refetch brings never reads as a change.
 */
export function systemKey(systems: MapSystemView[]): string {
	return systems
		.map(solarSystemId)
		.filter((id) => id !== null)
		.sort((a, b) => a - b)
		.join(',');
}
