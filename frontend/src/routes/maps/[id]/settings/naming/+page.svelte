<script lang="ts">
	// Naming: what the chain calls its systems, what that makes the bookmarks say, and
	// whether one lands on your clipboard as you map.
	//
	// Mixed ownership, unlike the other sections: the names are the map's and Manager+, the
	// clipboard is yours. Non-managers still see the names, because reading what your
	// bookmarks will say is worth doing even when you cannot change it.
	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { MapNaming } from '$lib/api/types/MapNaming';
	import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
	import type { MapView } from '$lib/api/types/MapView';
	import SettingRow from '$lib/components/settings/SettingRow.svelte';
	import * as Card from '$lib/components/ui/card';
	import { Switch } from '$lib/components/ui/switch';
	import NamingCard from './NamingCard.svelte';

	const mapId = $derived(Number(page.params.id) || 0);

	let view = $state<MapView | null>(null);
	let settings = $state<MapUserSettings | null>(null);
	let error = $state('');

	$effect(() => {
		if (!mapId) return;
		reload();
		api
			.mapUserSettings(mapId)
			.then((s) => (settings = s))
			.catch(() => {});
	});

	const canManage = $derived(view?.role === 'manager' || view?.role === 'owner');
	const tracking = $derived(settings?.tracking_allowed ?? false);

	async function reload() {
		try {
			view = await api.fetchMap(mapId);
		} catch (err) {
			error = (err as Error).message;
		}
	}

	async function saveNaming(naming: MapNaming) {
		try {
			await api.updateMap({ map_id: mapId, naming });
			error = '';
			await reload();
		} catch (err) {
			error = (err as Error).message;
		}
	}

	function update(patch: Record<string, unknown>) {
		api
			.updateMapUserSettings(mapId, patch)
			.then((s) => (settings = s))
			.catch(() => {});
	}
</script>

<div class="flex flex-col gap-6">
	{#if error}
		<p class="text-sm text-destructive" data-testid="settings-error">{error}</p>
	{/if}

	{#if view}
		<NamingCard naming={view.map.naming} disabled={!canManage} onsave={saveNaming} />
	{/if}

	<Card.Root>
		<Card.Header>
			<Card.Title>Copying</Card.Title>
			<Card.Description>What reaches your clipboard. Yours alone, per map.</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col py-0">
			<SettingRow
				id="copy-bookmark"
				label="Copy a bookmark when I map a hole"
				description="Puts the new connection's bookmark name on the clipboard, ready to paste into the in-game bookmark dialog."
				disabled={!tracking}
				blocked={tracking ? undefined : 'Needs location sharing, under Mapping.'}
			>
				{#snippet control()}
					<Switch
						checked={(settings?.copy_bookmark ?? false) && tracking}
						disabled={!tracking}
						aria-label="Copy a bookmark when I map a hole"
						onCheckedChange={(v) => update({ copy_bookmark: v })}
					/>
				{/snippet}
			</SettingRow>
		</Card.Content>
	</Card.Root>
</div>
