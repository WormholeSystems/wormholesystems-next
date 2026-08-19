<script lang="ts">
	// What the chain calls its systems, and what that makes the bookmarks say.
	//
	// Mixed ownership: the names are the map's and Manager+, the clipboard toggle is yours.
	// Non-managers still see the names read-only.
	import { invalidate } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { MapNaming } from '$lib/api/types/MapNaming';
	import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
	import type { MapView } from '$lib/api/types/MapView';
	import SettingRow from '$lib/components/settings/SettingRow.svelte';
	import * as Card from '$lib/components/ui/card';
	import { Switch } from '$lib/components/ui/switch';
	import NamingCard from './NamingCard.svelte';
	import { atLeast } from '$lib/map/roles';

	let { data }: { data: { view: MapView; settings: MapUserSettings | null } } = $props();

	const mapId = $derived(Number(page.params.id) || 0);
	let error = $state('');

	const canManage = $derived(atLeast(data.view.role, 'manager'));
	const tracking = $derived(data.settings?.tracking_allowed ?? false);

	async function saveNaming(naming: MapNaming) {
		try {
			await api.updateMap({ map_id: mapId, naming });
			error = '';
			await invalidate('vector:map');
		} catch (err) {
			error = (err as Error).message;
		}
	}

	function update(patch: Record<string, unknown>) {
		api
			.updateMapUserSettings(mapId, patch)
			.then(() => invalidate('vector:user-settings'))
			.catch(() => {});
	}
</script>

<div class="flex flex-col gap-6">
	{#if error}
		<p class="text-sm text-destructive" data-testid="settings-error">{error}</p>
	{/if}

	<NamingCard naming={data.view.map.naming} disabled={!canManage} onsave={saveNaming} />

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
						checked={(data.settings?.copy_bookmark ?? false) && tracking}
						disabled={!tracking}
						aria-label="Copy a bookmark when I map a hole"
						onCheckedChange={(v) => update({ copy_bookmark: v })}
					/>
				{/snippet}
			</SettingRow>
		</Card.Content>
	</Card.Root>
</div>
