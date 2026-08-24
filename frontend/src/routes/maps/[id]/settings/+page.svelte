<script lang="ts">
	// General: what the map is called and how to get rid of it. Everything here changes the
	// map for everyone on it, which is why it is Manager+ and why deletion sits at the
	// bottom behind its own confirmation.
	import { untrack } from 'svelte';
	import type { MapLayout } from '$lib/api/types/MapLayout';
	import { oneOf } from '$lib/enums';

	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { createQuery } from '@tanstack/svelte-query';
	import { goto } from '$app/navigation';
	import { browser } from '$app/environment';
	import { page } from '$app/state';
	import { toast } from 'svelte-sonner';

	import { api } from '$lib/api/client';
	import { apiAction } from '$lib/api/mutations';
	import { key, q } from '$lib/api/queries';
	import type { MapView } from '$lib/api/types/MapView';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';
	import SettingRow from '$lib/components/settings/SettingRow.svelte';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';
	import { atLeast } from '$lib/map/roles';

	let { data }: { data: { view: MapView } } = $props();

	const mapId = $derived(Number(page.params.id) || 0);
	// The layout's load is the first frame; after a save the cache owns the value, so
	// everything mutable reads the query, not `data`.
	const viewQuery = createQuery(() => ({ ...q.mapView(mapId), initialData: data.view }));
	const view = $derived(viewQuery.data);

	// Seeded from the map and re-seeded whenever it changes underneath, so a save landing
	// or somebody else's rename does not leave a stale draft in the fields.
	let name = $state(untrack(() => data.view.map.name));
	let description = $state(untrack(() => data.view.map.description ?? ''));
	$effect(() => {
		name = view.map.name;
		description = view.map.description ?? '';
	});

	const canManage = $derived(atLeast(view.role, 'manager'));
	const isOwner = $derived(view.role === 'owner');
	const dirty = $derived(
		name.trim() !== view.map.name || description.trim() !== (view.map.description ?? ''),
	);

	const act = apiAction(() => [key.mapView(mapId)]);

	const PLACEMENTS = [
		{ value: 'manual', label: 'Custom placement', hint: 'Everyone drags the chain into shape' },
		{ value: 'tree', label: 'Automatic placement', hint: 'Drawn as a tree from the connections' },
	] as const satisfies readonly { value: MapLayout; label: string; hint: string }[];
	const PLACEMENT_VALUES = PLACEMENTS.map((p) => p.value);
	const placement = $derived(view.map.layout);
	const allowOverride = $derived(view.map.allow_layout_override);

	function save() {
		if (!name.trim() || !dirty) return;
		act.mutate(() =>
			api.updateMap({
				map_id: mapId,
				name: name.trim(),
				description: description.trim() || null,
			}),
		);
	}

	// Who the map could be handed to: the characters already granted access.
	const accessQuery = createQuery(() => ({
		...q.listAccess(mapId),
		enabled: browser && mapId > 0 && isOwner,
	}));
	const access = $derived(accessQuery.data ?? []);
	let heir = $state('');
	const candidates = $derived(
		access.filter((e) => e.subject_type === 'character' && e.role !== 'owner'),
	);

	function transfer() {
		const subject = Number(heir);
		const heirName = candidates.find((c) => c.subject_id === subject)?.name ?? 'them';
		if (!subject || !confirm(`Hand "${view.map.name}" to ${heirName}? You stay on as a manager.`)) {
			return;
		}
		act
			.mutateAsync(() => api.transferOwnership({ map_id: mapId, subject_id: subject }))
			.then(() => {
				heir = '';
				toast.success(`${heirName} owns this map now`);
			})
			.catch(() => {});
	}

	const remove = apiAction(
		() => [key.maps],
		() => goto('/maps'),
	);

	function destroy() {
		if (!confirm(`Delete "${view?.map.name}"? This cannot be undone.`)) return;
		remove.mutate(() => api.deleteMap(mapId));
	}
</script>

<div class="flex flex-col gap-6">
	<Card.Root>
		<Card.Header>
			<Card.Title>Map</Card.Title>
			<Card.Description>What everyone on this map sees it called.</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col gap-4">
			<div class="flex flex-col gap-2">
				<label for="map-name" class="text-sm font-medium">Name</label>
				<Input
					id="map-name"
					bind:value={name}
					disabled={!canManage}
					data-testid="map-name-input"
					onkeydown={(e) => e.key === 'Enter' && save()}
				/>
			</div>
			<div class="flex flex-col gap-2">
				<label for="map-description" class="text-sm font-medium">Description</label>
				<Textarea
					id="map-description"
					bind:value={description}
					disabled={!canManage}
					rows={2}
					placeholder="What this chain is for, or where it stages from."
					data-testid="map-description-input"
				/>
			</div>
		</Card.Content>
		<Card.Footer>
			<Button
				variant="outline"
				disabled={!canManage || !name.trim() || !dirty}
				onclick={save}
				data-testid="rename-button">Save</Button
			>
		</Card.Footer>
	</Card.Root>

	<Card.Root>
		<Card.Header>
			<Card.Title>Placement</Card.Title>
			<Card.Description>How the chain is laid out for everyone on this map.</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col py-0">
			<SettingRow
				id="map-layout"
				label="Placement"
				description="Custom placement keeps the positions people dragged the systems to. Automatic ignores them and draws the chain as a tree from the connections, rooted at the pinned systems."
				disabled={!canManage}
				blocked={canManage ? undefined : 'Only a manager can change this.'}
			>
				{#snippet control()}
					<Select.Root
						type="single"
						value={placement}
						disabled={!canManage}
						onValueChange={(v) => {
							const picked = oneOf(PLACEMENT_VALUES, v);
							if (picked) act.mutate(() => api.updateMap({ map_id: mapId, layout: picked }));
						}}
					>
						<Select.Trigger class="w-56" data-testid="map-layout-select">
							{PLACEMENTS.find((p) => p.value === placement)?.label}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								{#each PLACEMENTS as option (option.value)}
									<Select.Item value={option.value} label={option.label}>
										<span class="flex flex-col">
											<span>{option.label}</span>
											<span class="text-xs text-muted-foreground">{option.hint}</span>
										</span>
									</Select.Item>
								{/each}
							</Select.Group>
						</Select.Content>
					</Select.Root>
				{/snippet}
			</SettingRow>

			<SettingRow
				id="allow-layout-override"
				label="Let people choose their own"
				description="Anyone on the map can switch their own view between the two, from the map itself. The map's own setting stays what everyone else sees."
				disabled={!canManage}
				blocked={canManage ? undefined : 'Only a manager can change this.'}
			>
				{#snippet control()}
					<Switch
						checked={allowOverride}
						disabled={!canManage}
						aria-label="Let people choose their own placement"
						onCheckedChange={(v) =>
							act.mutate(() => api.updateMap({ map_id: mapId, allow_layout_override: v }))}
					/>
				{/snippet}
			</SettingRow>
		</Card.Content>
	</Card.Root>

	<!-- The two things only an owner can do, and neither of them is undoable. -->
	{#if isOwner}
		<Card.Root class="border-destructive/40" data-testid="danger-zone">
			<Card.Header>
				<Card.Title>Danger zone</Card.Title>
				<Card.Description>Yours alone as the owner, and neither has an undo.</Card.Description>
			</Card.Header>
			<Card.Content class="flex flex-col py-0">
				<SettingRow
					id="transfer-ownership"
					label="Hand the map to someone else"
					description="The map becomes theirs: only they can transfer it again or delete it. You stay on as a manager. Pick anyone already granted access as a character."
					blocked={candidates.length === 0 ? 'Nobody else has access to this map yet.' : undefined}
				>
					{#snippet control()}
						<span class="flex items-center gap-2">
							<Select.Root type="single" bind:value={heir}>
								<Select.Trigger class="w-48" data-testid="transfer-target">
									{candidates.find((c) => String(c.subject_id) === heir)?.name ?? 'Pick a pilot'}
								</Select.Trigger>
								<Select.Content>
									<Select.Group>
										{#each candidates as c (c.subject_id)}
											<Select.Item
												value={String(c.subject_id)}
												label={c.name ?? String(c.subject_id)}
											>
												{c.name ?? c.subject_id}
											</Select.Item>
										{/each}
									</Select.Group>
								</Select.Content>
							</Select.Root>
							<Button
								variant="destructive"
								disabled={!heir}
								onclick={transfer}
								data-testid="transfer-button"
							>
								Transfer
							</Button>
						</span>
					{/snippet}
				</SettingRow>

				<SettingRow
					id="delete-map"
					label="Delete this map"
					description="Removes the map and everything on it, for everyone on it."
				>
					{#snippet control()}
						<Button variant="destructive" onclick={destroy} data-testid="delete-map">
							<TrashIcon data-icon="inline-start" />
							Delete map
						</Button>
					{/snippet}
				</SettingRow>
			</Card.Content>
		</Card.Root>
	{/if}
</div>
