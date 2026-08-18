<script lang="ts">
	// General: what the map is called and how to get rid of it. Everything here changes the
	// map for everyone on it, which is why it is Manager+ and why deletion sits at the
	// bottom behind its own confirmation.
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { MapView } from '$lib/api/types/MapView';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';

	const mapId = $derived(Number(page.params.id) || 0);

	let view = $state<MapView | null>(null);
	let name = $state('');
	let description = $state('');
	let error = $state('');

	const canManage = $derived(view?.role === 'manager' || view?.role === 'owner');
	const isOwner = $derived(view?.role === 'owner');
	const dirty = $derived(
		!!view &&
			(name.trim() !== view.map.name || description.trim() !== (view.map.description ?? ''))
	);

	$effect(() => {
		if (mapId) reload();
	});

	async function reload() {
		try {
			const v = await api.fetchMap(mapId);
			view = v;
			name = v.map.name;
			description = v.map.description ?? '';
		} catch (err) {
			error = (err as Error).message;
		}
	}

	async function act(work: Promise<unknown>) {
		try {
			await work;
			error = '';
			await reload();
		} catch (err) {
			error = (err as Error).message;
		}
	}

	function save() {
		if (!name.trim() || !dirty) return;
		act(
			api.updateMap({
				map_id: mapId,
				name: name.trim(),
				description: description.trim() || null
			})
		);
	}

	async function destroy() {
		if (!confirm(`Delete "${view?.map.name}"? This cannot be undone.`)) return;
		try {
			await api.deleteMap(mapId);
			goto('/maps');
		} catch (err) {
			error = (err as Error).message;
		}
	}
</script>

<div class="flex flex-col gap-6">
	{#if error}
		<p class="text-sm text-destructive" data-testid="settings-error">{error}</p>
	{/if}

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

	{#if isOwner}
		<Card.Root class="border-destructive/40">
			<Card.Header>
				<Card.Title>Delete this map</Card.Title>
				<Card.Description>
					Removes the map and everything on it for everyone. There is no undo.
				</Card.Description>
			</Card.Header>
			<Card.Footer>
				<Button variant="destructive" onclick={destroy} data-testid="delete-map">
					<TrashIcon data-icon="inline-start" />
					Delete map
				</Button>
			</Card.Footer>
		</Card.Root>
	{/if}
</div>
