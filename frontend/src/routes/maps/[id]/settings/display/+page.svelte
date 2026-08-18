<script lang="ts">
	// What the map shows you. Yours alone: two people on the same chain can disagree about
	// how dense the signature list should be without arguing about it.
	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
	import SettingRow from '$lib/components/settings/SettingRow.svelte';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';

	const mapId = $derived(Number(page.params.id) || 0);
	let settings = $state<MapUserSettings | null>(null);

	$effect(() => {
		if (!mapId) return;
		api
			.mapUserSettings(mapId)
			.then((s) => (settings = s))
			.catch(() => {});
	});

	function update(patch: Record<string, unknown>) {
		api
			.updateMapUserSettings(mapId, patch)
			.then((s) => (settings = s))
			.catch(() => {});
	}

	const FILTERS = [
		{ value: 'all', label: 'Everything' },
		{ value: 'jspace', label: 'Wormhole space only' },
		{ value: 'kspace', label: 'Known space only' }
	];
	const filter = $derived(settings?.killmail_filter ?? 'all');
</script>

<Card.Root>
	<Card.Header>
		<Card.Title>Display</Card.Title>
		<Card.Description>How this map looks to you, and nobody else.</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-col py-0">
		<SettingRow
			id="show-threat-level"
			label="Threat level on nodes"
			description="Colours a wormhole by how much has died there lately. Useful when scouting somewhere new, noise once you know the chain."
		>
			{#snippet control()}
				<Switch
					checked={settings?.show_threat_level ?? true}
					aria-label="Threat level on nodes"
					onCheckedChange={(v) => update({ show_threat_level: v })}
				/>
			{/snippet}
		</SettingRow>

		<SettingRow
			id="show-statics-first"
			label="Statics first in the wormhole list"
			description="Puts a system's own statics at the top of the type picker, where they are what you are almost always looking for."
		>
			{#snippet control()}
				<Switch
					checked={settings?.show_statics_first ?? false}
					aria-label="Statics first in the wormhole list"
					onCheckedChange={(v) => update({ show_statics_first: v })}
				/>
			{/snippet}
		</SettingRow>

		<SettingRow
			id="compact-signature-list"
			label="Compact signature list"
			description="Tighter rows, so a freshly scanned system fits without scrolling."
		>
			{#snippet control()}
				<Switch
					checked={settings?.compact_signature_list ?? false}
					aria-label="Compact signature list"
					onCheckedChange={(v) => update({ compact_signature_list: v })}
				/>
			{/snippet}
		</SettingRow>

		<SettingRow
			id="killmail-filter"
			label="Killmails to show"
			description="The card lists kills in the systems on this map. Narrow it to one half of the chain when the other half is drowning it out."
		>
			{#snippet control()}
				<Select.Root
					type="single"
					value={filter}
					onValueChange={(v) => v && update({ killmail_filter: v })}
				>
					<Select.Trigger class="w-52" data-testid="killmail-filter-select">
						{FILTERS.find((f) => f.value === filter)?.label}
					</Select.Trigger>
					<Select.Content>
						<Select.Group>
							{#each FILTERS as option (option.value)}
								<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
							{/each}
						</Select.Group>
					</Select.Content>
				</Select.Root>
			{/snippet}
		</SettingRow>
	</Card.Content>
</Card.Root>
