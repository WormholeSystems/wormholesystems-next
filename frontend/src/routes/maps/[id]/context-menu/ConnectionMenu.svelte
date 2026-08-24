<script lang="ts">
	// Right-clicking a connection: its degradable statuses, its kind, and removal.
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import CheckIcon from '@lucide/svelte/icons/check';
	import ClockIcon from '@lucide/svelte/icons/clock';
	import ShipIcon from '@lucide/svelte/icons/ship';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import WeightIcon from '@lucide/svelte/icons/weight';

	import { api } from '$lib/api/client';
	import type { ConnectionType } from '$lib/api/types/ConnectionType';
	import type { MapConnection } from '$lib/api/types/MapConnection';
	import type { MassStatus } from '$lib/api/types/MassStatus';
	import type { TimeStatus } from '$lib/api/types/TimeStatus';
	import type { WormholeSize } from '$lib/api/types/WormholeSize';
	import { LIFETIME_OPTIONS, MASS_OPTIONS, SIZE_OPTIONS } from '$lib/map/connection-status';
	import { patchConnection } from '$lib/map/connection-actions';
	import type { MapState } from '../map-state.svelte';
	import { item, panel, sub } from './chrome';

	let { map, connection }: { map: MapState; connection: MapConnection } = $props();

	const cid = $derived(connection.id);

	function close() {
		map.closeMenu();
	}

	function setKind(kind: ConnectionType) {
		patchConnection(map, cid, { kind });
		close();
	}

	function setMass(mass: MassStatus) {
		patchConnection(map, cid, { mass_status: mass });
		close();
	}

	function setLifetime(time: TimeStatus) {
		patchConnection(map, cid, { time_status: time });
		close();
	}

	function setSize(size: WormholeSize) {
		patchConnection(map, cid, { size });
		close();
	}

	function removeConnection() {
		map.run('removeConnection', api.removeConnection({ map_id: map.mapId, connection_id: cid }));
		close();
	}
</script>

{#snippet dot(color: string)}
	<span class="inline-block size-2 shrink-0 rounded-full" style="background-color: {color}"></span>
{/snippet}

{#snippet check(selected: boolean)}
	{#if selected}
		<CheckIcon class="size-3.5 shrink-0" />
	{/if}
{/snippet}

<div class={sub} data-testid="lifetime-subtrigger">
	<ClockIcon class="size-4" />
	Lifetime
	<ChevronRightIcon class="ml-auto size-3" />
	<div class={panel} data-testid="lifetime-submenu">
		{#each LIFETIME_OPTIONS as o (o.value)}
			<button class={item} onclick={() => setLifetime(o.value)}>
				{@render dot(o.color)}
				{o.label}
				<span class="ml-auto text-muted-foreground">{o.hint ?? ''}</span>
				{@render check(
					connection.time_status === o.value ||
						(o.value === 'stable' && connection.time_status === null),
				)}
			</button>
		{/each}
	</div>
</div>

<div class={sub} data-testid="mass-subtrigger">
	<WeightIcon class="size-4" />
	Mass Status
	<ChevronRightIcon class="ml-auto size-3" />
	<div class={panel} data-testid="mass-submenu">
		{#each MASS_OPTIONS as o (o.value)}
			<button class={item} onclick={() => setMass(o.value)}>
				{@render dot(o.color)}
				{o.label}
				<span class="ml-auto text-muted-foreground">{o.hint ?? ''}</span>
				{@render check(
					connection.mass_status === o.value ||
						(o.value === 'stable' && connection.mass_status === null),
				)}
			</button>
		{/each}
	</div>
</div>

<div class={sub} data-testid="size-subtrigger">
	<ShipIcon class="size-4" />
	Ship Size
	<ChevronRightIcon class="ml-auto size-3" />
	<div class={panel} data-testid="size-submenu">
		{#each SIZE_OPTIONS as o (o.value)}
			<button class={item} onclick={() => setSize(o.value)}>
				<span class="inline-flex w-6 justify-center font-mono text-[10px] text-muted-foreground">
					{o.letter}
				</span>
				{o.label}
				<span class="ml-auto"></span>
				{@render check(connection.size === o.value)}
			</button>
		{/each}
	</div>
</div>

<div class={sub} data-testid="type-subtrigger">
	<WaypointsIcon class="size-4" />
	Connection type
	<ChevronRightIcon class="ml-auto size-3" />
	<div class={panel} data-testid="type-submenu">
		<button class={item} onclick={() => setKind('wormhole')}>
			Wormhole
			<span class="ml-auto"></span>
			{@render check(connection.kind === 'wormhole')}
		</button>
		<button class={item} onclick={() => setKind('stargate')}>
			Stargate
			{#if connection.kind !== 'stargate'}
				<TriangleAlertIcon class="ml-auto size-3.5 text-amber-500" />
			{:else}
				<span class="ml-auto"></span>
				{@render check(true)}
			{/if}
		</button>
	</div>
</div>

<div class="my-0.5 border-t border-border"></div>
<button class="{item} text-destructive hover:text-destructive" onclick={removeConnection}>
	<Trash2Icon class="size-4" />
	Remove
</button>
