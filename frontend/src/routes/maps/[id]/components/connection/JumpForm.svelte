<script lang="ts">
	// The manual jump entry: a direction toggle, an optional ship (which fills the mass), and
	// the mass itself. Mounted fresh per open, so the buffer seeds itself from `editing`.
	import MoveLeftIcon from '@lucide/svelte/icons/move-left';
	import MoveRightIcon from '@lucide/svelte/icons/move-right';
	import XIcon from '@lucide/svelte/icons/x';

	import type { ConnectionJump } from '$lib/api/types/ConnectionJump';
	import type { JumpDirection } from '$lib/api/types/JumpDirection';
	import type { MapConnection } from '$lib/api/types/MapConnection';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { ShipSearchResult } from '$lib/api/types/ShipSearchResult';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import EveImage from '$lib/components/EveImage.svelte';
	import ShipCombobox from '../pickers/ShipCombobox.svelte';
	import { solarSystemId, systemName } from '$lib/map/system';
	import type { MapState } from '../../state/map-state.svelte';

	let {
		map,
		connection,
		source,
		target,
		editing,
		ondone,
		oncancel,
	}: {
		map: MapState;
		connection: MapConnection;
		source: MapSystemView;
		target: MapSystemView;
		editing: ConnectionJump | null;
		ondone: () => void;
		oncancel: () => void;
	} = $props();

	// Seeded once on purpose: this is an editing buffer, and the popover remounts the form
	// every time it opens.
	/* svelte-ignore state_referenced_locally */
	const seed = editing;
	/* svelte-ignore state_referenced_locally */
	const seedOutbound = seed === null || seed.from_solar_system_id === solarSystemId(source);

	let direction = $state<JumpDirection>(seedOutbound ? 'outbound' : 'inbound');
	let shipTypeId = $state<number | null>(seed?.ship_type_id ?? null);
	let shipLabel = $state(seed?.ship_type_name ?? '');
	let massKt = $state(seed === null ? '' : String(Math.round((seed.mass / 1_000_000) * 10) / 10));

	function systemLabel(s: MapSystemView): string {
		return s.alias ?? systemName(s) ?? 'Unmapped';
	}
	const directionLabel = $derived(
		direction === 'outbound'
			? `${systemLabel(source)} → ${systemLabel(target)}`
			: `${systemLabel(target)} → ${systemLabel(source)}`,
	);

	function pickShip(result: ShipSearchResult) {
		shipTypeId = result.id;
		shipLabel = result.name;
		if (result.mass) massKt = String(Math.round((result.mass / 1_000_000) * 10) / 10);
	}

	const massKg = $derived.by(() => {
		const parsed = Number(massKt);
		if (massKt === '' || !Number.isFinite(parsed) || parsed < 0) return undefined;
		return Math.round(parsed * 1_000_000);
	});
	const canSubmit = $derived(massKg !== undefined || shipTypeId !== null);

	function submitForm() {
		if (!canSubmit) return;
		if (editing) {
			void map.connections
				.updateJump({
					jump_pk: editing.id,
					direction,
					ship_type_id: shipTypeId,
					...(massKg !== undefined ? { mass: massKg } : {}),
				})
				.then(ondone, () => {});
		} else {
			void map.connections
				.addJump({
					connection_id: connection.id,
					direction,
					...(shipTypeId !== null ? { ship_type_id: shipTypeId } : {}),
					...(massKg !== undefined ? { mass: massKg } : {}),
				})
				.then(ondone, () => {});
		}
	}
</script>

<form
	class="space-y-2"
	onsubmit={(ev) => {
		ev.preventDefault();
		submitForm();
	}}
>
	<div class="flex items-center justify-between">
		<span class="text-[10px] font-medium tracking-wider text-muted-foreground uppercase">
			{editing ? 'Edit jump' : 'Log jump'}
		</span>
		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<button
						{...props}
						type="button"
						class="flex min-w-0 items-center gap-1 rounded text-xs text-muted-foreground transition-colors hover:text-foreground"
						data-testid="jump-direction"
						onclick={() => (direction = direction === 'outbound' ? 'inbound' : 'outbound')}
					>
						{#if direction === 'outbound'}
							<MoveRightIcon class="size-3 shrink-0 text-sky-500" />
						{:else}
							<MoveLeftIcon class="size-3 shrink-0 text-purple-500" />
						{/if}
						<span class="truncate">{directionLabel}</span>
					</button>
				{/snippet}
			</Tooltip.Trigger>
			<Tooltip.Content>Click to flip the jump direction</Tooltip.Content>
		</Tooltip.Root>
	</div>

	{#if shipTypeId !== null}
		<div class="flex items-center gap-1.5 rounded-md border px-2 py-1">
			<EveImage kind="type" id={shipTypeId} class="size-4 rounded" />
			<span class="min-w-0 flex-1 truncate text-xs">{shipLabel}</span>
			<button
				type="button"
				aria-label="Clear ship"
				class="text-muted-foreground hover:text-foreground"
				onclick={() => {
					shipTypeId = null;
					shipLabel = '';
				}}
			>
				<XIcon class="size-3" />
			</button>
		</div>
	{:else}
		<ShipCombobox onpick={pickShip} />
	{/if}

	<div class="flex items-center gap-2">
		<Input
			bind:value={massKt}
			type="number"
			min="0"
			step="any"
			placeholder="Mass"
			class="h-7 flex-1 text-xs"
			data-testid="jump-mass"
		/>
		<span class="text-xs text-muted-foreground">kt</span>
		<Button type="submit" size="xs" disabled={!canSubmit}>Save</Button>
		<Button type="button" size="xs" variant="ghost" onclick={oncancel}>Cancel</Button>
	</div>
</form>
