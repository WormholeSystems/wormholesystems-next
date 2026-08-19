<script lang="ts">
	// What the map does for you while you fly: whether it may follow you, and how much of
	// the mapping it fills in when you jump a hole.
	//
	// Location sharing gates the rest, and is shown as such rather than left to be
	// discovered by toggling something that then does nothing.
	//
	// The scanning card above it is the map's, not yours: an unmapped hole put on the map
	// is a node everyone sees, so it cannot be one person's preference.
	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
	import type { MapView } from '$lib/api/types/MapView';
	import type { ScopeStatus } from '$lib/api/types/ScopeStatus';
	import SettingRow from '$lib/components/settings/SettingRow.svelte';
	import * as Card from '$lib/components/ui/card';
	import { Switch } from '$lib/components/ui/switch';
	import { atLeast } from '$lib/map/roles';

	const LOCATION_SCOPE = 'esi-location.read_location.v1';

	const mapId = $derived(Number(page.params.id) || 0);
	let settings = $state<MapUserSettings | null>(null);
	let scopes = $state<ScopeStatus[]>([]);
	let view = $state<MapView | null>(null);

	$effect(() => {
		if (!mapId) return;
		api
			.mapUserSettings(mapId)
			.then((s) => (settings = s))
			.catch(() => {});
		api
			.myScopes()
			.then((s) => (scopes = s))
			.catch(() => {});
		reload();
	});

	const hasLocation = $derived(scopes.some((s) => s.scope === LOCATION_SCOPE && s.granted));
	const tracking = $derived(settings?.tracking_allowed ?? false);
	const canManage = $derived(atLeast(view?.role, 'manager'));
	const ghosting = $derived(view?.map.ghost_unlinked_wormholes ?? false);

	async function reload() {
		try {
			view = await api.fetchMap(mapId);
		} catch {
			// The per-user settings below still work without it.
		}
	}

	function update(patch: Record<string, unknown>) {
		api
			.updateMapUserSettings(mapId, patch)
			.then((s) => (settings = s))
			.catch(() => {});
	}

	function updateMap(ghost: boolean) {
		api
			.updateMap({ map_id: mapId, ghost_unlinked_wormholes: ghost })
			.then(() => reload())
			.catch(() => {});
	}
</script>

<div class="flex flex-col gap-6">
<Card.Root>
	<Card.Header>
		<Card.Title>Scanning</Card.Title>
		<Card.Description>What a pasted scan puts on the map, for everyone on it.</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-col py-0">
		<SettingRow
			id="ghost-unlinked-wormholes"
			label="Put unmapped wormholes on the map"
			description="A pasted wormhole signature that is not on the map yet gets a node hanging off the system it was scanned in, with no system behind it. Lay the chain out and name it before anyone flies it; assign the system from the node's menu once someone has."
			disabled={!canManage}
			blocked={canManage ? undefined : 'Only a manager can change this.'}
		>
			{#snippet control()}
				<Switch
					checked={ghosting}
					disabled={!canManage}
					aria-label="Put unmapped wormholes on the map"
					onCheckedChange={(v) => updateMap(v)}
				/>
			{/snippet}
		</SettingRow>
	</Card.Content>
</Card.Root>

<Card.Root>
	<Card.Header>
		<Card.Title>Mapping</Card.Title>
		<Card.Description>
			What happens automatically as you fly the chain. Yours alone, per map.
		</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-col py-0">
		<SettingRow
			id="tracking-allowed"
			label="Share my location on this map"
			description="Puts you on your system for everyone here, measures distances from where you actually are, and lets the map build the chain as you jump. Revocable at any time."
			blocked={hasLocation ? undefined : 'Needs the character location permission from EVE.'}
		>
			{#snippet control()}
				<Switch
					checked={tracking && hasLocation}
					disabled={!hasLocation}
					aria-label="Share my location on this map"
					onCheckedChange={(v) => update({ tracking_allowed: v })}
				/>
			{/snippet}
		</SettingRow>

		<SettingRow
			id="prompt-for-signature"
			label="Ask which signature I jumped"
			description="On arriving somewhere new, the map asks which signature the hole was and links it. Without this the connection is still drawn, just unnamed."
			disabled={!tracking}
			blocked={tracking ? undefined : 'Needs location sharing.'}
		>
			{#snippet control()}
				<Switch
					checked={(settings?.prompt_for_signature ?? true) && tracking}
					disabled={!tracking}
					aria-label="Ask which signature I jumped"
					onCheckedChange={(v) => update({ prompt_for_signature: v })}
				/>
			{/snippet}
		</SettingRow>

		<SettingRow
			id="suggest-alias"
			label="Name new systems for me"
			description="Fills in the next alias from the chain's naming scheme, so holes are named the same way by everyone on the map."
			disabled={!tracking}
			blocked={tracking ? undefined : 'Needs location sharing.'}
		>
			{#snippet control()}
				<Switch
					checked={(settings?.suggest_alias ?? true) && tracking}
					disabled={!tracking}
					aria-label="Name new systems for me"
					onCheckedChange={(v) => update({ suggest_alias: v })}
				/>
			{/snippet}
		</SettingRow>
	</Card.Content>
</Card.Root>
</div>
