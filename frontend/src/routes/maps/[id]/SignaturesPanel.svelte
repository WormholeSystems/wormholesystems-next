<script lang="ts">
	// The signatures panel (legacy Signatures.vue port): sortable column table with
	// category filters, compact mode, clipboard-driven paste with diff tints and lazy
	// delete, and per-row editing. Scoped to the active system.
	import ClipboardPasteIcon from '@lucide/svelte/icons/clipboard-paste';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Rows2Icon from '@lucide/svelte/icons/rows-2';
	import Rows3Icon from '@lucide/svelte/icons/rows-3';
	import TrashIcon from '@lucide/svelte/icons/trash-2';
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';

	import { browser } from '$app/environment';
	import { api } from '$lib/api/client';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import type { PastedSignature } from '$lib/api/types/PastedSignature';
	import type { Signature } from '$lib/api/types/Signature';
	import type { SignatureCatalog } from '$lib/api/types/SignatureCatalog';
	import { Button } from '$lib/components/ui/button';
	import * as ToggleGroup from '$lib/components/ui/toggle-group';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import { CATEGORIES, loadCatalog, parseScan, typeById } from '$lib/map/signatures';
	import type { MapState } from './map-state.svelte';
	import MismatchDialog from './signatures/MismatchDialog.svelte';
	import SignatureRow from './signatures/SignatureRow.svelte';

	let { map, system, layoutActions }: {
		map: MapState;
		system: MapSystemView;
		layoutActions?: import('svelte').Snippet;
	} = $props();

	let catalog = $state<SignatureCatalog | null>(null);
	$effect(() => {
		loadCatalog().then((c) => (catalog = c));
	});

	const canWrite = $derived((map.data?.role ?? 'viewer') !== 'viewer');
	const compact = $derived(map.userSettings?.compact_signature_list ?? false);
	const showStaticsFirst = $derived(map.userSettings?.show_statics_first ?? false);

	// Category filters: the HIDDEN set persists, so new categories default to visible.
	let hidden = $state<string[]>(
		browser ? JSON.parse(localStorage.getItem('signatures-category-hidden-filters') ?? '[]') : []
	);
	$effect(() => {
		localStorage.setItem('signatures-category-hidden-filters', JSON.stringify(hidden));
	});
	const activeFilters = $derived(
		CATEGORIES.map((c) => c.group as string).filter((g) => !hidden.includes(g))
	);

	// Sorting (legacy default: id desc; ties by id ascending; nulls last).
	type SortColumn = 'id' | 'category' | 'type' | 'age';
	let sort = $state<{ column: SortColumn; direction: 'asc' | 'desc' }>(
		(browser && JSON.parse(localStorage.getItem('signatures-sort') ?? 'null')) || {
			column: 'id',
			direction: 'desc'
		}
	);
	$effect(() => {
		localStorage.setItem('signatures-sort', JSON.stringify(sort));
	});
	function handleSort(column: SortColumn) {
		sort =
			sort.column === column
				? { column, direction: sort.direction === 'asc' ? 'desc' : 'asc' }
				: { column, direction: 'asc' };
	}

	const mySigs = $derived(map.sigs.filter((s) => s.solar_system_id === system.solar_system_id));
	const filtered = $derived(mySigs.filter((s) => !hidden.includes(s.group)));
	const hiddenCount = $derived(mySigs.length - filtered.length);

	function typeName(s: Signature): string | null {
		return (catalog && typeById(catalog, s.signature_type_id)?.name) ?? s.name;
	}
	// Wormhole ages run from creation; site ages from the last update (legacy).
	function modifiedDate(s: Signature): number {
		return Date.parse(s.group === 'wormhole' ? s.created_at : s.updated_at);
	}
	function cmpNullableStrings(a: string | null, b: string | null): number {
		if (a === null && b === null) return 0;
		if (a === null) return 1;
		if (b === null) return -1;
		return a.localeCompare(b);
	}

	const sorted = $derived.by(() => {
		const dir = sort.direction === 'asc' ? 1 : -1;
		return filtered.toSorted((a, b) => {
			let cmp = 0;
			switch (sort.column) {
				case 'id':
					cmp = a.signature_id.localeCompare(b.signature_id);
					break;
				case 'category':
					cmp = cmpNullableStrings(
						a.group === 'unknown' ? null : a.group,
						b.group === 'unknown' ? null : b.group
					);
					break;
				case 'type':
					cmp = cmpNullableStrings(typeName(a), typeName(b));
					break;
				case 'age':
					// Newest first in ascending order (legacy quirk).
					cmp = modifiedDate(b) - modifiedDate(a);
					break;
			}
			if (cmp !== 0) return cmp * dir;
			return a.signature_id.localeCompare(b.signature_id);
		});
	});

	// --- Paste selection + diff ---
	// The pre-paste ids are snapshotted so new (green) vs updated (amber) stays stable
	// after the server round-trip creates the new rows.
	let pasted = $state<PastedSignature[] | null>(null);
	let preIds = $state<Set<string>>(new Set());
	let pending = $state<PastedSignature[] | null>(null);
	let mismatchOpen = $state(false);
	let mismatchSystem = $state('Unknown');

	// Refetches hand the panel a fresh `system` object, so guard on the actual id: the
	// paste selection only clears when the active system really changes.
	let lastSystemId = $state<number | null>(null);
	$effect(() => {
		if (system.id !== lastSystemId) {
			lastSystemId = system.id;
			pasted = null;
			pending = null;
		}
	});

	const pastedIds = $derived(
		pasted === null ? null : new Set(pasted.map((p) => p.signature_id))
	);
	const deletedSigs = $derived(
		pastedIds === null ? [] : mySigs.filter((s) => !pastedIds.has(s.signature_id))
	);
	function rowStatus(s: Signature): 'new' | 'updated' | 'deleted' | null {
		if (pastedIds === null) return null;
		if (!pastedIds.has(s.signature_id)) return 'deleted';
		return preIds.has(s.signature_id) ? 'updated' : 'new';
	}

	async function handlePasteText(text: string) {
		if (!canWrite) return;
		const rows = parseScan(text, await loadCatalog());
		if (rows.length === 0) {
			map.statusLine = 'no signatures parsed';
			return;
		}
		const active = map.myCharacters.find((c) => c.is_active);
		if (
			active?.solar_system_id != null &&
			active.solar_system_id !== system.solar_system_id
		) {
			pending = rows;
			api
				.resolveSystems([active.solar_system_id])
				.then((r) => (mismatchSystem = r[0]?.name ?? 'Unknown'))
				.catch(() => (mismatchSystem = 'Unknown'))
				.finally(() => (mismatchOpen = true));
			return;
		}
		commitPaste(rows);
	}

	function commitPaste(rows: PastedSignature[]) {
		preIds = new Set(mySigs.map((s) => s.signature_id));
		pasted = rows;
		map.run(
			'paste sigs',
			api.pasteSignatures({
				map_id: map.mapId,
				solar_system_id: system.solar_system_id,
				signatures: rows
			})
		);
	}

	function onWindowPaste(e: ClipboardEvent) {
		if (!canWrite) return;
		const el = e.target instanceof HTMLElement ? e.target : null;
		if (el?.closest('input, textarea, [contenteditable]')) return;
		const text = e.clipboardData?.getData('text/plain');
		if (!text) return;
		e.preventDefault();
		handlePasteText(text);
	}

	async function pasteFromClipboard() {
		if (!navigator.clipboard?.readText) {
			map.statusLine = 'clipboard access is not available in this browser';
			return;
		}
		try {
			handlePasteText(await navigator.clipboard.readText());
		} catch {
			map.statusLine = 'clipboard access denied';
		}
	}

	function deleteMissing() {
		map.run(
			'delete missing sigs',
			api.removeSignaturesBulk({
				map_id: map.mapId,
				signature_pks: deletedSigs.map((s) => s.id)
			})
		);
		pasted = null;
	}

	// --- New signature (inline row; saved once a valid 7-char id is typed) ---
	let creating = $state(false);
	let newId = $state('');
	let newInput = $state<HTMLInputElement | null>(null);

	function startCreate() {
		creating = true;
		newId = '';
		setTimeout(() => newInput?.focus());
	}

	function formatId(raw: string): string {
		const clean = raw.replace(/[^a-zA-Z0-9]/g, '').toUpperCase();
		return clean.length >= 4 ? `${clean.slice(0, 3)}-${clean.slice(3, 6)}` : clean;
	}

	function saveNew() {
		const value = newId.trim();
		creating = false;
		newId = '';
		if (value.length === 7) {
			map.run(
				'add sig',
				api.addSignature({
					map_id: map.mapId,
					solar_system_id: system.solar_system_id,
					signature_id: value,
					group: 'unknown'
				})
			);
		}
	}

	async function setSetting(patch: Record<string, boolean>) {
		map.userSettings = await api.updateMapUserSettings(map.mapId, patch);
	}
</script>

<svelte:window onpaste={onWindowPaste} />

<MapPanel testid="signatures-card">
	<MapPanelHeader>
		Signatures
		{#if filtered.length > 0}
			<span class="ml-1 text-amber-400">{filtered.length}</span>
		{/if}
		{#if hiddenCount > 0}
			<span class="ml-1 text-muted-foreground/70">{hiddenCount} hidden</span>
		{/if}
		{#snippet actions()}
			{@render layoutActions?.()}
			<Button
				variant="ghost"
				size="icon"
				class="size-6 text-muted-foreground hover:text-foreground"
				aria-label={compact
					? 'Switch to comfortable signature list'
					: 'Switch to compact signature list'}
				title={compact ? 'Switch to comfortable signature list' : 'Switch to compact signature list'}
				data-testid="compact-toggle"
				onclick={() => setSetting({ compact_signature_list: !compact })}
			>
				{#if compact}
					<Rows2Icon class="size-3.5" />
				{:else}
					<Rows3Icon class="size-3.5" />
				{/if}
			</Button>
			<ToggleGroup.Root
				type="multiple"
				size="sm"
				variant="outline"
				value={activeFilters}
				onValueChange={(values) => {
					hidden = CATEGORIES.map((c) => c.group as string).filter((g) => !values.includes(g));
				}}
			>
				{#each CATEGORIES as c (c.group)}
					<ToggleGroup.Item
						value={c.group}
						aria-label={c.label}
						title={c.label}
						class="size-6 min-w-0"
						data-testid="filter-{c.group}"
					>
						<c.icon class="size-3 {c.color}" />
					</ToggleGroup.Item>
				{/each}
			</ToggleGroup.Root>
			{#if canWrite}
				{#if pasted !== null}
					<Button
						variant="ghost"
						class="h-6 px-2 text-[11px] leading-none"
						title="Unselect signatures"
						onclick={() => (pasted = null)}
					>
						Unselect
					</Button>
				{/if}
				{#if deletedSigs.length > 0}
					<Button
						variant="destructive"
						size="icon"
						class="size-6"
						aria-label="Delete missing signatures"
						title="Delete missing signatures and their connections"
						data-testid="delete-missing"
						onclick={deleteMissing}
					>
						<TrashIcon class="size-3.5" />
					</Button>
				{/if}
				<Button
					variant="ghost"
					size="icon"
					class="size-6 text-muted-foreground hover:text-foreground"
					aria-label="Paste signatures"
					title="Paste signatures from clipboard (Ctrl/Cmd + V)"
					data-testid="paste-clipboard"
					onclick={pasteFromClipboard}
				>
					<ClipboardPasteIcon class="size-3.5" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					class="size-6 text-muted-foreground hover:text-foreground"
					aria-label="Create new signature"
					title="Create new signature"
					data-testid="new-signature"
					onclick={startCreate}
				>
					<PlusIcon class="size-3.5" />
				</Button>
			{/if}
		{/snippet}
	</MapPanelHeader>
	<MapPanelContent>
		<Tooltip.Provider delayDuration={300}>
			<!-- Column header (always rendered, even with zero rows). -->
			<div
				class="flex items-center gap-2 border-b border-border/30 bg-muted/20 px-3 font-mono text-[10px] tracking-wider text-muted-foreground uppercase {compact
					? 'py-0.5'
					: 'py-1.5'}"
			>
				{#snippet arrow(column: SortColumn)}
					{#if sort.column === column}
						{#if sort.direction === 'asc'}
							<ArrowUpIcon class="size-3" />
						{:else}
							<ArrowDownIcon class="size-3" />
						{/if}
					{/if}
				{/snippet}
				<button
					class="flex w-16 shrink-0 items-center gap-1 hover:text-foreground"
					onclick={() => handleSort('id')}>ID {@render arrow('id')}</button
				>
				<button
					class="flex w-20 shrink-0 items-center gap-1 hover:text-foreground"
					onclick={() => handleSort('category')}>Cat {@render arrow('category')}</button
				>
				<button
					class="flex min-w-0 flex-1 items-center gap-1 hover:text-foreground"
					onclick={() => handleSort('type')}>Type {@render arrow('type')}</button
				>
				<span class="min-w-0 flex-1">Conn</span>
				<button
					class="flex w-10 shrink-0 items-center justify-end gap-1 hover:text-foreground"
					onclick={() => handleSort('age')}>Age {@render arrow('age')}</button
				>
				<span class="w-12 shrink-0"></span>
			</div>

			{#if creating}
				<div
					class="flex items-center gap-2 border-b border-border/30 px-3 {compact
						? 'py-0.5'
						: 'py-1.5'}"
				>
					<div class="w-16 shrink-0">
						<input
							bind:this={newInput}
							value={newId}
							oninput={(e) => (newId = formatId(e.currentTarget.value))}
							onblur={saveNew}
							onkeydown={(e) => {
								if (e.key === 'Enter') saveNew();
								if (e.key === 'Escape') {
									creating = false;
									newId = '';
								}
							}}
							class="w-full rounded border border-border/50 bg-background/50 px-1.5 font-mono text-xs uppercase focus:border-primary focus:outline-none {compact
								? 'h-5'
								: 'h-6'}"
							maxlength="7"
							placeholder="XXX-XXX"
							data-testid="new-signature-id"
						/>
					</div>
					<span class="text-xs text-muted-foreground">New signature</span>
				</div>
			{/if}

			{#if catalog && sorted.length > 0}
				{#each sorted as sig (sig.id)}
					<SignatureRow
						{map}
						{system}
						{sig}
						{catalog}
						{compact}
						{canWrite}
						{showStaticsFirst}
						status={rowStatus(sig)}
					/>
				{/each}
			{:else if !creating}
				<div class="flex flex-col items-center justify-center gap-2 p-4">
					<p class="font-mono text-[10px] tracking-wider text-muted-foreground/60 uppercase">
						{hiddenCount > 0 ? `${hiddenCount} hidden by filters` : 'No signatures'}
					</p>
				</div>
			{/if}
		</Tooltip.Provider>
	</MapPanelContent>
</MapPanel>

<MismatchDialog
	bind:open={mismatchOpen}
	targetLabel={system.alias ? `${system.alias} (${system.name})` : system.name}
	characterSystem={mismatchSystem}
	onconfirm={() => {
		if (pending !== null) commitPaste(pending);
		pending = null;
	}}
	oncancel={() => (pending = null)}
/>
