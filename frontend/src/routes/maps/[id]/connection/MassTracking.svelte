<script lang="ts">
	// Mass tracking (legacy MassTracking): remaining-mass bar with 10%/50% ticks, the
	// jump log in a nested popover, and the manual log/edit form in a third. The bar is
	// an estimate from tracked hull masses; the manual mass-status flag is independent.
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import EllipsisVerticalIcon from '@lucide/svelte/icons/ellipsis-vertical';
	import MoveLeftIcon from '@lucide/svelte/icons/move-left';
	import MoveRightIcon from '@lucide/svelte/icons/move-right';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import PencilLineIcon from '@lucide/svelte/icons/pencil-line';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import XIcon from '@lucide/svelte/icons/x';

	import { api } from '$lib/api/client';
	import type { ConnectionJump } from '$lib/api/types/ConnectionJump';
	import type { JumpDirection } from '$lib/api/types/JumpDirection';
	import type { MapConnection } from '$lib/api/types/MapConnection';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { ShipSearchResult } from '$lib/api/types/ShipSearchResult';
	import type { SignatureTypeInfo } from '$lib/api/types/SignatureTypeInfo';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { Input } from '$lib/components/ui/input';
	import ShipCombobox from '$lib/components/pickers/ShipCombobox.svelte';
	import * as Popover from '$lib/components/ui/popover';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import EveImage from '$lib/components/EveImage.svelte';
	import { formatKt } from '$lib/map/helpers';
	import type { MapState } from '../map-state.svelte';

	let {
		map,
		connection,
		source,
		target,
		physics,
		canWrite
	}: {
		map: MapState;
		connection: MapConnection;
		source: MapSystemView;
		target: MapSystemView;
		physics: SignatureTypeInfo | null;
		canWrite: boolean;
	} = $props();

	const totalMass = $derived(physics?.total_mass ?? null);
	const remainingPercent = $derived.by(() => {
		if (totalMass === null || totalMass <= 0) return null;
		return Math.max(0, 100 - (connection.jumps_mass_sum / totalMass) * 100);
	});
	const remainingMassKg = $derived(
		totalMass === null ? null : Math.max(0, totalMass - connection.jumps_mass_sum)
	);
	const barColor = $derived.by(() => {
		if (remainingPercent === null) return 'bg-neutral-500';
		if (remainingPercent <= 10) return 'bg-red-500';
		if (remainingPercent <= 50) return 'bg-amber-500';
		return 'bg-green-500';
	});

	// --- jump log ---
	let logOpen = $state(false);
	let jumps = $state<ConnectionJump[]>([]);
	async function refreshLog() {
		try {
			jumps = await api.listConnectionJumps(map.mapId, connection.id);
		} catch {
			jumps = [];
		}
	}
	$effect(() => {
		if (logOpen) refreshLog();
	});
	// New transits arrive via the map refetch; keep the open log in sync.
	$effect(() => {
		void connection.jumps_count;
		void connection.jumps_mass_sum;
		if (logOpen) refreshLog();
	});

	function isOutbound(jump: ConnectionJump): boolean {
		return jump.from_solar_system_id === source.solar_system_id;
	}

	let now = $state(Date.now());
	$effect(() => {
		const t = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(t);
	});
	function jumpedAgo(jump: ConnectionJump): string {
		const mins = Math.floor((now - Date.parse(jump.created_at)) / 60_000);
		if (mins < 1) return 'now';
		if (mins < 60) return `${mins}m`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours}h`;
		return `${Math.floor(hours / 24)}d`;
	}
	function jumpedAt(jump: ConnectionJump): string {
		return new Date(jump.created_at).toLocaleString('en-US', {
			month: 'short',
			day: '2-digit',
			hour: '2-digit',
			minute: '2-digit',
			hour12: false,
			timeZone: 'UTC'
		});
	}

	// --- manual jump form (add / edit) ---
	let formOpen = $state(false);
	let editing = $state<ConnectionJump | null>(null);
	let direction = $state<JumpDirection>('outbound');
	let shipTypeId = $state<number | null>(null);
	let shipLabel = $state('');
	let massKt = $state('');

	function systemLabel(s: MapSystemView): string {
		return s.alias ?? s.name;
	}
	const directionLabel = $derived(
		direction === 'outbound'
			? `${systemLabel(source)} → ${systemLabel(target)}`
			: `${systemLabel(target)} → ${systemLabel(source)}`
	);

	function openAdd() {
		editing = null;
		direction = 'outbound';
		shipTypeId = null;
		shipLabel = '';
		massKt = '';
		formOpen = true;
	}

	function openEdit(jump: ConnectionJump) {
		editing = jump;
		direction = isOutbound(jump) ? 'outbound' : 'inbound';
		shipTypeId = jump.ship_type_id;
		shipLabel = jump.ship_type_name ?? '';
		massKt = String(Math.round((jump.mass / 1_000_000) * 10) / 10);
		// Deferred: the closing dropdown's focus restore would dismiss a popover
		// opened in the same tick (legacy has the same workaround).
		setTimeout(() => (formOpen = true));
	}

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
		const done = () => {
			formOpen = false;
			refreshLog();
		};
		if (editing) {
			map.run(
				'jump',
				api
					.updateConnectionJump({
						map_id: map.mapId,
						jump_pk: editing.id,
						direction,
						ship_type_id: shipTypeId,
						...(massKg !== undefined ? { mass: massKg } : {})
					})
					.then(done)
			);
		} else {
			map.run(
				'jump',
				api
					.addConnectionJump({
						map_id: map.mapId,
						connection_id: connection.id,
						direction,
						...(shipTypeId !== null ? { ship_type_id: shipTypeId } : {}),
						...(massKg !== undefined ? { mass: massKg } : {})
					})
					.then(done)
			);
		}
	}

	function deleteJump(jump: ConnectionJump) {
		map.run(
			'rm jump',
			api.removeConnectionJump({ map_id: map.mapId, jump_pk: jump.id }).then(refreshLog)
		);
	}
</script>

<div class="space-y-1" data-testid="mass-tracking">
	<div class="flex items-center justify-between border-b pb-1 text-xs font-medium text-foreground">
		<Tooltip.Root>
			<Tooltip.Trigger class="cursor-help">Mass (estimate)</Tooltip.Trigger>
			<Tooltip.Content>
				Only tracked pilots are counted automatically; the total mass varies by ±10% in game.
			</Tooltip.Content>
		</Tooltip.Root>

		<Popover.Root bind:open={logOpen}>
			<Popover.Trigger
				class="flex items-center gap-0.5 rounded font-normal text-muted-foreground transition-colors hover:text-foreground"
				data-testid="jump-log-trigger"
			>
				{connection.jumps_count} jumps
				<ChevronRightIcon class="size-3" />
			</Popover.Trigger>
			<Popover.Content class="w-96 p-0" side="right" align="start" data-testid="jump-log">
				<div class="max-h-64 overflow-y-auto px-3">
					<div
						class="grid grid-cols-[auto_auto_1fr_auto_auto_auto_auto] divide-y divide-border/40"
					>
						<div
							class="sticky top-0 z-10 col-span-full grid grid-cols-subgrid gap-x-3 bg-popover py-1.5 text-[10px] font-medium tracking-wider text-muted-foreground uppercase"
						>
							<span class="col-span-3">Ship</span>
							<span>Pilot</span>
							<span class="text-right">kt</span>
							<span class="text-right">Age</span>
							{#if canWrite}
								<button
									class="flex items-center justify-end text-muted-foreground transition-colors hover:text-foreground"
									title="Log jump manually"
									aria-label="Log jump manually"
									data-testid="log-jump"
									onclick={openAdd}
								>
									<PlusIcon class="size-3" />
								</button>
							{:else}
								<span></span>
							{/if}
						</div>

						{#if jumps.length === 0}
							<div class="col-span-full py-3 text-center text-xs text-muted-foreground">
								No jumps logged yet
							</div>
						{/if}
						{#each jumps as jump (jump.id)}
							<div
								class="group col-span-full grid grid-cols-subgrid items-center gap-x-3 py-1.5 transition-colors hover:bg-muted/30"
								data-testid="jump-row"
							>
								{#if isOutbound(jump)}
									<MoveRightIcon class="size-3 shrink-0 text-sky-500" />
								{:else}
									<MoveLeftIcon class="size-3 shrink-0 text-purple-500" />
								{/if}
								<div class="size-5 shrink-0">
									{#if jump.ship_type_id !== null}
										<EveImage kind="type" id={jump.ship_type_id} class="size-5 rounded" />
									{/if}
								</div>
								<span class="min-w-0 truncate text-xs text-foreground">
									{jump.ship_type_name ?? 'Unknown'}
								</span>
								{#if jump.character_id !== null}
									<span class="flex items-center gap-1">
										<EveImage
											kind="character"
											id={jump.character_id}
											class="size-4 shrink-0 rounded-full"
										/>
										<span class="max-w-20 truncate text-[10px] text-muted-foreground">
											{jump.character_name}
										</span>
									</span>
								{:else}
									<div class="flex items-center gap-1 text-[10px] text-muted-foreground italic">
										<PencilLineIcon class="size-3 shrink-0" />
										manual
									</div>
								{/if}
								<span
									class="text-right font-mono text-[10px] whitespace-nowrap text-foreground/80 tabular-nums"
								>
									{formatKt(jump.mass)}
								</span>
								<Tooltip.Root>
									<Tooltip.Trigger
										class="cursor-help text-right font-mono text-[10px] whitespace-nowrap text-muted-foreground tabular-nums"
									>
										{jumpedAgo(jump)}
									</Tooltip.Trigger>
									<Tooltip.Content>{jumpedAt(jump)}</Tooltip.Content>
								</Tooltip.Root>
								{#if canWrite}
									<DropdownMenu.Root>
										<DropdownMenu.Trigger
											class="flex items-center justify-end text-muted-foreground hover:text-foreground"
											title="Jump actions"
											aria-label="Jump actions"
										>
											<EllipsisVerticalIcon class="size-3" />
										</DropdownMenu.Trigger>
										<DropdownMenu.Content side="right" align="start">
											<DropdownMenu.Item class="text-xs" onclick={() => openEdit(jump)}>
												<PencilIcon class="size-3.5" />
												Edit
											</DropdownMenu.Item>
											<DropdownMenu.Item
												class="text-xs text-destructive focus:text-destructive"
												onclick={() => deleteJump(jump)}
											>
												<Trash2Icon class="size-3.5" />
												Delete
											</DropdownMenu.Item>
										</DropdownMenu.Content>
									</DropdownMenu.Root>
								{:else}
									<span></span>
								{/if}
							</div>
						{/each}
					</div>
				</div>
				<div class="flex items-center justify-between border-t bg-muted/30 px-3 py-2 text-xs">
					<span class="font-medium text-foreground">
						Total
						{#if connection.jumps_count > jumps.length}
							<span class="font-normal text-muted-foreground">
								· latest {jumps.length} of {connection.jumps_count} shown
							</span>
						{/if}
					</span>
					<span class="font-mono text-foreground tabular-nums">
						{formatKt(connection.jumps_mass_sum)} kt
					</span>
				</div>

				<!-- Manual log / edit form (nested, matching legacy). -->
				<Popover.Root bind:open={formOpen}>
					<Popover.Trigger class="pointer-events-none absolute top-0 right-0" tabindex={-1} />
					<Popover.Content
						class="w-72 p-3"
						side="right"
						align="start"
						data-testid="jump-form"
						onpointerdown={(ev: PointerEvent) => ev.stopPropagation()}
					>
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
												onclick={() =>
													(direction = direction === 'outbound' ? 'inbound' : 'outbound')}
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
								<Button type="button" size="xs" variant="ghost" onclick={() => (formOpen = false)}>
									Cancel
								</Button>
							</div>
						</form>
					</Popover.Content>
				</Popover.Root>
			</Popover.Content>
		</Popover.Root>
	</div>

	<div class="grid grid-cols-2 divide-y text-xs text-muted-foreground *:py-1">
		{#if remainingPercent !== null}
			<div class="col-span-full">
				<div class="relative h-1.5 w-full overflow-hidden rounded-full bg-muted">
					<div
						class="h-full rounded-full transition-all {barColor}"
						style="width: {remainingPercent}%"
						data-testid="mass-bar"
					></div>
					<Tooltip.Root>
						<Tooltip.Trigger
							class="absolute inset-y-0 left-[10%] flex w-2 -translate-x-1/2 justify-center"
						>
							<span class="h-full w-px bg-popover"></span>
						</Tooltip.Trigger>
						<Tooltip.Content>Below 10% the hole verges to critical</Tooltip.Content>
					</Tooltip.Root>
					<Tooltip.Root>
						<Tooltip.Trigger
							class="absolute inset-y-0 left-1/2 flex w-2 -translate-x-1/2 justify-center"
						>
							<span class="h-full w-px bg-popover"></span>
						</Tooltip.Trigger>
						<Tooltip.Content>Below 50% the hole shrinks to reduced</Tooltip.Content>
					</Tooltip.Root>
				</div>
			</div>
			{#if remainingMassKg !== null}
				<div class="col-span-full grid grid-cols-subgrid">
					<span>Remaining</span>
					<span class="text-right tabular-nums" data-testid="mass-remaining">
						≈ {formatKt(remainingMassKg)} ({Math.round(remainingPercent)}%)
					</span>
				</div>
			{/if}
		{/if}
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Jumped</span>
			<span class="text-right tabular-nums" data-testid="mass-jumped">
				{formatKt(connection.jumps_mass_sum)}
			</span>
		</div>
	</div>
</div>
