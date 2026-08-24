<script lang="ts">
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import UsersIcon from '@lucide/svelte/icons/users';
	import type { Snippet } from 'svelte';

	import { api } from '$lib/api/client';
	import type { RouteStep } from '$lib/routing/algorithm';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as Popover from '$lib/components/ui/popover';
	import EveImage from '$lib/components/EveImage.svelte';
	import RouteList from '$lib/components/map/RouteList.svelte';
	import { onlineCharacters, setWaypoint, setWaypointAll } from '$lib/map/waypoints';
	import type { MapState } from '../map-state.svelte';

	let {
		map,
		steps,
		children,
	}: {
		map: MapState;
		steps: RouteStep[];
		children: Snippet;
	} = $props();

	const jumps = $derived(Math.max(0, steps.length - 1));
	const destinationId = $derived(steps.at(-1)?.id ?? null);
	const online = $derived(onlineCharacters(map));
</script>

<Popover.Root>
	<Popover.Trigger
		class="justify-self-end"
		data-testid="jump-badge"
		onclick={(ev: MouseEvent) => ev.stopPropagation()}
	>
		{@render children()}
	</Popover.Trigger>
	<!-- Same as the connection popover: without this the auto-focus lands on the first
	     tooltip trigger in the list and pops a tooltip nobody asked for. -->
	<Popover.Content
		class="w-[26rem] gap-0 p-0"
		align="end"
		data-testid="route-popover"
		onOpenAutoFocus={(ev) => ev.preventDefault()}
	>
		<div
			class="flex items-center justify-between gap-2 border-b border-border/50 bg-muted/30 px-3 py-2"
		>
			<span class="font-mono text-[10px] tracking-wider text-muted-foreground uppercase">
				Route · {jumps} jumps
			</span>
			{#if destinationId !== null && online.length > 0}
				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="secondary"
								size="sm"
								class="h-6 shrink-0 gap-1 px-2 text-[10px]"
							>
								<NavigationIcon class="size-3" />
								Set Destination
							</Button>
						{/snippet}
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="end">
						{#each online as c (c.character_id)}
							<DropdownMenu.Item
								class="text-xs"
								onclick={() => setWaypoint(map, destinationId, c.character_id, true)}
							>
								<EveImage
									kind="character"
									id={c.character_id}
									size={32}
									class="size-5 rounded-lg"
								/>
								{c.name}
							</DropdownMenu.Item>
						{/each}
						{#if online.length > 1}
							<DropdownMenu.Separator />
							<DropdownMenu.Item
								class="text-xs"
								onclick={() => setWaypointAll(map, destinationId, true)}
							>
								<UsersIcon class="size-4" />
								All Characters
							</DropdownMenu.Item>
						{/if}
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			{/if}
		</div>
		<div class="p-2">
			<RouteList
				steps={map.route.withSignatures(steps)}
				onignore={(id) => map.route.ignoreSystem(id)}
			/>
		</div>
	</Popover.Content>
</Popover.Root>
