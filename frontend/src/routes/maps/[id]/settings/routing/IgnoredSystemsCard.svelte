<script lang="ts">
	// The map's ignore list: what everyone routes around and nobody maps by flying. The
	// list is the map's and Manager+; everyone else reads it, since a route that skips a
	// system is easier to trust when you can see why.
	import XIcon from '@lucide/svelte/icons/x';

	import { api } from '$lib/api/client';
	import { apiAction } from '$lib/api/mutations';
	import { key } from '$lib/api/queries';
	import type { IgnoredSystem } from '$lib/api/types/IgnoredSystem';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { systemResolver } from '$lib/resolve-cache.svelte';
	import SystemCombobox from '../../components/pickers/SystemCombobox.svelte';

	let {
		mapId,
		ignored,
		canManage,
	}: { mapId: number; ignored: IgnoredSystem[]; canManage: boolean } = $props();

	const act = apiAction(() => [key.ignored(mapId)]);

	$effect(() => {
		systemResolver.ensure(ignored.map((row) => row.solar_system_id));
	});

	const rows = $derived(
		ignored
			.map((row) => ({ id: row.solar_system_id, system: systemResolver.get(row.solar_system_id) }))
			.sort((a, b) => (a.system?.name ?? '').localeCompare(b.system?.name ?? '')),
	);

	function add(id: number | null) {
		if (id === null) return;
		act.mutate(() => api.addIgnored({ map_id: mapId, solar_system_id: id }));
	}
</script>

<Card.Root>
	<Card.Header>
		<Card.Title>Ignored systems</Card.Title>
		<Card.Description>
			For everyone on the map. Routes and jump counts go around these, alerts never route through
			them, and mapping as you fly will not place them. Trade hubs and the systems you pass through
			on the way to somewhere else belong here.
		</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-col gap-3">
		{#if canManage}
			<div class="flex w-72 items-center">
				<SystemCombobox placeholder="Add a system" value={null} onpick={add} />
			</div>
		{/if}

		{#if rows.length === 0}
			<p class="text-xs text-muted-foreground" data-testid="ignored-empty">
				No systems are ignored.
			</p>
		{:else}
			<ul class="flex flex-col divide-y divide-border/50 text-sm" data-testid="ignored-list">
				{#each rows as row (row.id)}
					<li class="flex items-center gap-2 py-1.5" data-testid="ignored-row">
						<ClassBadge
							classId={row.system?.wormhole_class_id ?? null}
							security={row.system?.security ?? 0}
						/>
						<span class="truncate">{row.system?.name ?? row.id}</span>
						{#if row.system}
							<span class="truncate text-xs text-muted-foreground">{row.system.region}</span>
						{/if}
						{#if canManage}
							<Button
								variant="ghost"
								size="icon-xs"
								class="ml-auto"
								aria-label="Stop ignoring {row.system?.name ?? row.id}"
								onclick={() =>
									act.mutate(() => api.removeIgnored({ map_id: mapId, solar_system_id: row.id }))}
							>
								<XIcon />
							</Button>
						{/if}
					</li>
				{/each}
			</ul>
			{#if canManage}
				<Button
					variant="ghost"
					size="sm"
					class="self-end text-muted-foreground hover:text-destructive"
					data-testid="ignored-clear"
					onclick={() => act.mutate(() => api.clearIgnored({ map_id: mapId }))}
				>
					Clear the list
				</Button>
			{/if}
		{/if}
	</Card.Content>
</Card.Root>
