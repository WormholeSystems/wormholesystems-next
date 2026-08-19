<script lang="ts">
	// One signature row (legacy Signature.vue): inline-editable ID, category select,
	// type/connection inputs, age cell, copy-bookmark and the overflow menu. Paste-diff
	// status tints via data attributes.
	import CopyIcon from '@lucide/svelte/icons/copy';
	import MoreVerticalIcon from '@lucide/svelte/icons/more-vertical';
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { toast } from 'svelte-sonner';

	import { api } from '$lib/api/client';
	import { formatBookmark } from '$lib/bookmark';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { Signature } from '$lib/api/types/Signature';
	import type { SignatureCatalog } from '$lib/api/types/SignatureCatalog';
	import type { SignatureGroup } from '$lib/api/types/SignatureGroup';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as Select from '$lib/components/ui/select';
	import { destClassMeta } from '$lib/map/classes';
	import { CATEGORIES, categoryMeta, typeById } from '$lib/map/signatures';
	import type { MapState } from '../map-state.svelte';
	import ConnectionInput from './ConnectionInput.svelte';
	import TimeDetails from './TimeDetails.svelte';
	import TypeInput from './TypeInput.svelte';

	let {
		map,
		system,
		sig,
		catalog,
		compact,
		canWrite,
		showStaticsFirst,
		status
	}: {
		map: MapState;
		system: MapSystemView;
		sig: Signature;
		catalog: SignatureCatalog;
		compact: boolean;
		canWrite: boolean;
		showStaticsFirst: boolean;
		status: 'new' | 'updated' | 'deleted' | null;
	} = $props();

	const isWormhole = $derived(sig.group === 'wormhole');
	const cat = $derived(categoryMeta(sig.group));

	const connection = $derived(
		sig.connection_id === null
			? null
			: (map.connections.find((c) => c.id === sig.connection_id) ?? null)
	);
	// The linked connection's far end, for type narrowing and the bookmark class.
	const linkedTarget = $derived.by(() => {
		if (!connection) return null;
		const otherPid =
			connection.from_system === system.id ? connection.to_system : connection.from_system;
		return map.systems.find((s) => s.id === otherPid) ?? null;
	});

	// --- Inline ID editing (legacy: alnum only, uppercased, dash after 3 chars) ---
	let editingId = $state(false);
	let idDraft = $state('');
	let idInput = $state<HTMLInputElement | null>(null);

	function startEditId() {
		if (!canWrite) return;
		idDraft = sig.signature_id;
		editingId = true;
		setTimeout(() => {
			idInput?.focus();
			idInput?.select();
		});
	}

	function formatId(raw: string): string {
		const clean = raw.replace(/[^a-zA-Z0-9]/g, '').toUpperCase();
		return clean.length >= 4 ? `${clean.slice(0, 3)}-${clean.slice(3, 6)}` : clean;
	}

	function saveId() {
		const value = idDraft.trim();
		editingId = false;
		if (value.length === 7 && value !== sig.signature_id) {
			update({ signature_id: value });
		}
	}

	function update(patch: Record<string, unknown>) {
		map.run(
			'sig update',
			api.updateSignature({ map_id: map.mapId, signature_pk: sig.id, ...patch })
		);
	}

	function pickCategory(value: string) {
		const group = value as SignatureGroup;
		if (group === sig.group) return;
		update({ group });
	}

	// The bookmark names the *far* end of the hole: it is filed in game as "where this
	// leads", not as where you are standing. Until the hole is mapped the only thing known
	// about that end is the class the signature type promises.
	function copyBookmark() {
		const type = typeById(catalog, sig.signature_type_id);
		const far = linkedTarget;
		const text = formatBookmark(
			{
				alias: far?.alias ?? null,
				// Blank rather than borrowing this system's: an unmapped hole's far side is
				// genuinely unknown, and the class the type promises is all we can say.
				name: far?.name ?? '',
				region: far?.region ?? null,
				wormholeClassId: far?.wormhole_class_id ?? type?.target_class ?? null,
				security: far?.security_status ?? null,
				occupier: far?.occupying_group ?? null
			},
			{
				signatureId: sig.signature_id,
				size: sig.size,
				massStatus: sig.mass_status,
				timeStatus: sig.time_status,
				wormholeCode: type?.signature ?? null
			},
			null,
			system.alias
		);
		navigator.clipboard?.writeText(text);
		toast.success('Bookmark copied', { description: text });
	}

	function remove() {
		map.run('rm sig', api.removeSignature({ map_id: map.mapId, signature_pk: sig.id }));
	}

	function togglePreserveMass() {
		if (!connection) return;
		map.run(
			'preserve mass',
			api.setConnectionStatus({
				map_id: map.mapId,
				connection_id: connection.id,
				preserve_mass: !connection.preserve_mass
			})
		);
	}

	const MASS_OPTIONS = [
		{ value: 'unknown', label: 'Fresh Mass', dot: 'bg-neutral-500' },
		{ value: 'reduced', label: 'Reduced Mass', dot: 'bg-amber-500' },
		{ value: 'critical', label: 'Critical Mass', dot: 'bg-red-500' }
	];
	const LIFETIME_OPTIONS = [
		{ value: 'stable', label: 'Healthy', dot: 'bg-neutral-500' },
		{ value: 'eol', label: 'End of Life', dot: 'bg-purple-500' },
		{ value: 'critical', label: 'Critical', dot: 'bg-red-500' }
	];
</script>

<div
	class="flex items-center gap-2 border-b border-border/30 px-3 hover:bg-muted/30 data-[status=deleted]:bg-red-500/10 data-[status=new]:bg-green-500/10 data-[status=updated]:bg-amber-500/15 {compact
		? 'py-0.5'
		: 'py-1.5'}"
	data-testid="sig-row"
	data-sig={sig.signature_id}
	data-status={status}
>
	<!-- ID -->
	<div class="w-16 shrink-0">
		{#if editingId}
			<input
				bind:this={idInput}
				value={idDraft}
				oninput={(e) => (idDraft = formatId(e.currentTarget.value))}
				onblur={saveId}
				onkeydown={(e) => {
					if (e.key === 'Enter') saveId();
					if (e.key === 'Escape') editingId = false;
				}}
				class="w-full rounded border border-border/50 bg-background/50 px-1.5 font-mono text-xs uppercase focus:border-primary focus:outline-none {compact
					? 'h-5'
					: 'h-6'}"
				maxlength="7"
				placeholder="XXX-XXX"
			/>
		{:else}
			<button
				class="flex items-center font-mono text-xs hover:text-amber-400 {canWrite
					? 'cursor-pointer'
					: 'cursor-default'} {compact ? 'h-5' : 'h-6'}"
				onclick={startEditId}
			>
				{sig.signature_id || '---'}
			</button>
		{/if}
	</div>

	<!-- Category -->
	<div class="w-20 shrink-0">
		<Select.Root
			type="single"
			value={sig.group === 'unknown' ? '' : sig.group}
			onValueChange={pickCategory}
			disabled={!canWrite}
		>
			<Select.Trigger class="w-full min-w-0 overflow-hidden text-xs {compact ? '!h-5 !py-0' : ''}" data-testid="sig-category">
				{#if sig.group === 'unknown'}
					<span class="text-muted-foreground">Category</span>
				{:else}
					<span class="flex min-w-0 items-center gap-1">
						<cat.icon class="size-3 shrink-0 {cat.color}" />
						<span class="truncate">{cat.abbrev}</span>
					</span>
				{/if}
			</Select.Trigger>
			<Select.Content>
				<Select.Group>
					{#each CATEGORIES.filter((c) => c.categoryId !== null) as c (c.group)}
						<Select.Item value={c.group} class="text-xs" label={c.label}>
							<span class="flex items-center gap-1.5">
								<c.icon class="size-3 shrink-0 {c.color}" />
								{c.label}
							</span>
						</Select.Item>
					{/each}
				</Select.Group>
			</Select.Content>
		</Select.Root>
	</div>

	<!-- Type -->
	<div class="min-w-0 flex-1 {isWormhole ? 'max-w-44' : ''}">
		<TypeInput
			{system}
			{sig}
			{catalog}
			{compact}
			{canWrite}
			{showStaticsFirst}
			linkedClass={linkedTarget?.wormhole_class_id ?? null}
			onpick={(typeId) => update({ signature_type_id: typeId })}
		/>
	</div>

	<!-- Connection (wormhole rows only; sites let the type cell absorb the column) -->
	{#if isWormhole}
		<div class="min-w-0 flex-1">
			<ConnectionInput {map} {system} {sig} {catalog} {compact} {canWrite} />
		</div>
	{/if}

	<!-- Age -->
	<div class="w-10 shrink-0 text-right">
		<TimeDetails {sig} {connection} {compact} />
	</div>

	<!-- Actions -->
	<div class="flex w-12 shrink-0 items-center justify-end gap-1">
		{#if isWormhole}
			<Button
				variant="ghost"
				size="icon"
				class="size-6 text-muted-foreground hover:text-foreground"
				aria-label="Copy bookmark"
				onclick={copyBookmark}
			>
				<CopyIcon class="size-3.5" />
			</Button>
		{/if}
		{#if canWrite}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger>
					{#snippet child({ props })}
						<Button
							{...props}
							variant="ghost"
							size="icon"
							class="text-muted-foreground hover:text-foreground {compact ? 'size-5' : 'size-6'}"
							aria-label="Signature menu"
						>
							<MoreVerticalIcon class="size-3.5" />
						</Button>
					{/snippet}
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" class="w-44">
					{#if isWormhole}
						<DropdownMenu.RadioGroup
							value={sig.mass_status ?? 'unknown'}
							onValueChange={(v) => update({ mass_status: v === 'unknown' ? null : v })}
						>
							{#each MASS_OPTIONS as opt (opt.value)}
								<DropdownMenu.RadioItem value={opt.value} class="text-xs">
									<span class="flex items-center gap-2">
										<span class="inline-block size-2 rounded-full {opt.dot}"></span>
										{opt.label}
									</span>
								</DropdownMenu.RadioItem>
							{/each}
						</DropdownMenu.RadioGroup>
						<DropdownMenu.Separator />
						<DropdownMenu.RadioGroup
							value={sig.time_status ?? 'stable'}
							onValueChange={(v) => update({ time_status: v === 'stable' ? null : v })}
						>
							{#each LIFETIME_OPTIONS as opt (opt.value)}
								<DropdownMenu.RadioItem value={opt.value} class="text-xs">
									<span class="flex items-center gap-2">
										<span class="inline-block size-2 rounded-full {opt.dot}"></span>
										{opt.label}
									</span>
								</DropdownMenu.RadioItem>
							{/each}
						</DropdownMenu.RadioGroup>
						<DropdownMenu.Separator />
						{#if connection}
							<DropdownMenu.CheckboxItem
								checked={connection.preserve_mass}
								onCheckedChange={togglePreserveMass}
								class="text-xs"
							>
								Preserve mass
							</DropdownMenu.CheckboxItem>
							<DropdownMenu.Separator />
						{/if}
					{/if}
					<DropdownMenu.Item
						class="text-xs text-destructive focus:text-destructive"
						onclick={remove}
					>
						<TrashIcon class="mr-2 size-3.5" />
						Delete Signature
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		{/if}
	</div>
</div>
