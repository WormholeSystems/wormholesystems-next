<script lang="ts">
	// What the map shows you, per viewer. Placement is the exception: the mode is the map's,
	// and the row only appears when the map hands the choice to each viewer.
	import { page } from '$app/state';
	import { saveUserSettings } from '$lib/map/user-settings';
	import { api } from '$lib/api/client';
	import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
	import type { MapView } from '$lib/api/types/MapView';
	import SettingRow from '$lib/components/settings/SettingRow.svelte';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';

	let { data }: { data: { view: MapView; settings: MapUserSettings | null } } = $props();

	const mapId = $derived(Number(page.params.id) || 0);
	const settings = $derived(data.settings);
	const view = $derived(data.view);

	const PLACEMENTS = [
		{ value: 'map', label: 'Follow the map' },
		{ value: 'manual', label: 'Custom placement' },
		{ value: 'tree', label: 'Automatic placement' }
	];
	const placement = $derived(settings?.layout_override ?? 'map');


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
					onCheckedChange={(v) => saveUserSettings(mapId, { show_threat_level: v })}
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
					onCheckedChange={(v) => saveUserSettings(mapId, { show_statics_first: v })}
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
					onCheckedChange={(v) => saveUserSettings(mapId, { compact_signature_list: v })}
				/>
			{/snippet}
		</SettingRow>

		{#if view?.map.allow_layout_override}
			<SettingRow
				id="layout-override"
				label="How this chain is placed"
				description="Custom placement is the map as people dragged it. Automatic draws it as a tree from the connections, which nobody can move. Following the map takes whichever it is set to."
			>
				{#snippet control()}
					<Select.Root
						type="single"
						value={placement}
						onValueChange={(v) =>
							v && saveUserSettings(mapId, { layout_override: v === 'map' ? null : v })}
					>
						<Select.Trigger class="w-52" data-testid="layout-override-select">
							{PLACEMENTS.find((p) => p.value === placement)?.label}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								{#each PLACEMENTS as option (option.value)}
									<Select.Item value={option.value} label={option.label}>
										{option.label}
									</Select.Item>
								{/each}
							</Select.Group>
						</Select.Content>
					</Select.Root>
				{/snippet}
			</SettingRow>
		{/if}

		<SettingRow
			id="killmail-filter"
			label="Killmails to show"
			description="The card lists kills in the systems on this map. Narrow it to one half of the chain when the other half is drowning it out."
		>
			{#snippet control()}
				<Select.Root
					type="single"
					value={filter}
					onValueChange={(v) => v && saveUserSettings(mapId, { killmail_filter: v })}
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
