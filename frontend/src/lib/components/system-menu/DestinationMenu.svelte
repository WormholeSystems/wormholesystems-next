<script lang="ts">
	// A minimal right-click menu for any ESI destination that is not a solar system
	// (stations, structures): set destination / add waypoint per online character.
	// Solar systems use the richer SystemMenu instead.
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import UsersIcon from '@lucide/svelte/icons/users';
	import { getContext, type Snippet } from 'svelte';

	import { api } from '$lib/api/client';
	import * as ContextMenu from '$lib/components/ui/context-menu';
	import EveImage from '$lib/components/EveImage.svelte';
	import type { MapState } from '../../../routes/maps/[id]/map-state.svelte';

	let {
		destinationId,
		class: cls = 'contents',
		children
	}: {
		/** An ESI destination id: station, structure, or system. */
		destinationId: number;
		class?: string;
		children: Snippet;
	} = $props();

	const getMap = getContext<(() => MapState) | undefined>('map-state');
	const map = $derived(getMap?.());
	const onlineCharacters = $derived(map?.myCharacters.filter((c) => c.online) ?? []);

	function waypoint(characterId: number, clearOthers: boolean) {
		map?.run(
			'waypoint',
			api.setWaypoint({
				character_id: characterId,
				destination_id: destinationId,
				clear_other_waypoints: clearOthers
			})
		);
	}

	function waypointAll(clearOthers: boolean) {
		map?.run(
			'waypoint',
			api.setWaypointAll({ destination_id: destinationId, clear_other_waypoints: clearOthers })
		);
	}
</script>

{#snippet waypointItems(clearOthers: boolean)}
	{#if onlineCharacters.length === 0}
		<ContextMenu.Item disabled>No characters online</ContextMenu.Item>
	{:else}
		{#each onlineCharacters as c (c.character_id)}
			<ContextMenu.Item onclick={() => waypoint(c.character_id, clearOthers)}>
				<EveImage kind="character" id={c.character_id} size={32} class="size-5 rounded-lg" />
				{c.name}
			</ContextMenu.Item>
		{/each}
		{#if onlineCharacters.length > 1}
			<ContextMenu.Separator />
			<ContextMenu.Item onclick={() => waypointAll(clearOthers)}>
				<UsersIcon class="size-4" />
				All Characters
			</ContextMenu.Item>
		{/if}
	{/if}
{/snippet}

<ContextMenu.Root>
	<ContextMenu.Trigger class={cls} data-testid="destination-menu-trigger">
		{@render children()}
	</ContextMenu.Trigger>
	<ContextMenu.Content class="w-52" data-testid="destination-menu">
		{#if map !== undefined}
			<ContextMenu.Sub>
				<ContextMenu.SubTrigger data-testid="menu-set-destination">
					<NavigationIcon class="size-4" />
					Set destination
				</ContextMenu.SubTrigger>
				<ContextMenu.SubContent class="w-48">
					{@render waypointItems(true)}
				</ContextMenu.SubContent>
			</ContextMenu.Sub>
			<ContextMenu.Sub>
				<ContextMenu.SubTrigger data-testid="menu-add-waypoint">
					<MapPinIcon class="size-4" />
					Add waypoint
				</ContextMenu.SubTrigger>
				<ContextMenu.SubContent class="w-48">
					{@render waypointItems(false)}
				</ContextMenu.SubContent>
			</ContextMenu.Sub>
		{:else}
			<ContextMenu.Item disabled>No map context</ContextMenu.Item>
		{/if}
	</ContextMenu.Content>
</ContextMenu.Root>
