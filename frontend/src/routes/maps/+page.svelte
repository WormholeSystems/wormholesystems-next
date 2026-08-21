<script lang="ts">
	// Every map you can reach, ordered by when the chain last changed: the map you want is
	// nearly always the one you were just in.
	import ArchiveIcon from '@lucide/svelte/icons/archive';
	import PinIcon from '@lucide/svelte/icons/pin';
	import PinOffIcon from '@lucide/svelte/icons/pin-off';
	import ArchiveRestoreIcon from '@lucide/svelte/icons/archive-restore';
	import MoreVerticalIcon from '@lucide/svelte/icons/more-vertical';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import SearchIcon from '@lucide/svelte/icons/search';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';

	import { goto, invalidateAll } from '$app/navigation';
	import { toast } from 'svelte-sonner';

	import { api, errorMessage } from '$lib/api/client';
	import type { MapEntry } from '$lib/api/types/MapEntry';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Textarea } from '$lib/components/ui/textarea';
	import { timeAgo } from '$lib/format';
	import { cn } from '$lib/utils';

	let { data }: { data: { maps: MapEntry[] } } = $props();

	const maps = $derived(data.maps);
	let error = $state('');
	let query = $state('');
	let showArchived = $state(false);
	let now = $state(new Date());

	let creating = $state(false);
	let newName = $state('');
	let newDescription = $state('');
	let busy = $state(false);

	// The whole entry, not just an id, so the dialog can name what it is about to destroy.
	let deleting = $state<MapEntry | null>(null);

	$effect(() => {
		// "4m ago" has to stay true while the page is open.
		const clock = setInterval(() => (now = new Date()), 30_000);
		return () => clearInterval(clock);
	});

	const matching = $derived.by(() => {
		const rows = maps;
		const q = query.trim().toLowerCase();
		if (!q) return rows;
		return rows.filter(
			(m) => m.name.toLowerCase().includes(q) || (m.description ?? '').toLowerCase().includes(q),
		);
	});

	/** Most recently touched first; a map nobody has changed yet falls back to its age. */
	function byRecency(a: MapEntry, b: MapEntry) {
		const at = new Date(a.last_activity ?? a.created_at).getTime();
		const bt = new Date(b.last_activity ?? b.created_at).getTime();
		return bt - at;
	}

	const active = $derived(matching.filter((m) => !m.is_archived).sort(byRecency));
	const archived = $derived(matching.filter((m) => m.is_archived).sort(byRecency));

	const totals = $derived({
		maps: active.length,
		systems: active.reduce((n, m) => n + m.system_count, 0),
		pilots: active.reduce((n, m) => n + m.pilots_online, 0),
	});

	async function create() {
		const name = newName.trim();
		if (!name || busy) return;
		busy = true;
		try {
			const map = await api.createMap(name, newDescription.trim() || undefined);
			creating = false;
			newName = '';
			newDescription = '';
			await goto(`/maps/${map.id}`);
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	// The list and the top bar's shortcuts both come from loads, so one refresh does both.
	async function setPinned(map: MapEntry, value: boolean) {
		try {
			await api.updateMapUserSettings(map.id, { is_pinned: value });
			await invalidateAll();
			toast.success(value ? `${map.name} pinned to the top bar` : `${map.name} unpinned`);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function setArchived(map: MapEntry, value: boolean) {
		try {
			await api.updateMapUserSettings(map.id, { is_archived: value });
			await invalidateAll();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function confirmDelete() {
		const map = deleting;
		if (!map) return;
		deleting = null;
		try {
			await api.deleteMap(map.id);
			await invalidateAll();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	const ROLE_TONE: Record<string, string> = {
		owner: 'text-amber-500',
		manager: 'text-purple-400',
		member: 'text-emerald-500',
		viewer: 'text-muted-foreground',
	};
</script>

{#snippet card(map: MapEntry)}
	<div
		class={cn(
			'group relative flex flex-col border border-border bg-card transition-colors hover:border-foreground/25',
			map.is_archived && 'opacity-60',
		)}
		data-testid="map-card"
		data-map={map.name}
	>
		<a href="/maps/{map.id}" class="flex flex-1 flex-col gap-2 p-3">
			<span class="flex items-start justify-between gap-2">
				<span class="min-w-0">
					<span class="block truncate font-heading text-sm font-semibold">{map.name}</span>
					{#if map.description}
						<span class="mt-0.5 block truncate text-xs text-muted-foreground">
							{map.description}
						</span>
					{/if}
				</span>
			</span>

			<span
				class="mt-auto flex items-center gap-3 font-mono text-[10px] tracking-wider text-muted-foreground uppercase"
			>
				<span class="tabular-nums">{map.system_count} sys</span>
				<span class="tabular-nums">{map.connection_count} conn</span>
				<span class="tabular-nums"
					>{map.member_count} member{map.member_count === 1 ? '' : 's'}</span
				>
			</span>
		</a>

		<div class="flex items-center justify-between border-t border-border/50 px-3 py-1.5">
			<span class="flex items-center gap-2">
				<Badge variant="outline" class={cn('h-4 px-1 text-[10px] uppercase', ROLE_TONE[map.role])}>
					{map.role}
				</Badge>
				{#if map.pilots_online > 0}
					<span
						class="flex items-center gap-1 font-mono text-[10px] text-emerald-500"
						data-testid="map-pilots"
					>
						<span class="size-1.5 animate-pulse rounded-full bg-emerald-500"></span>
						{map.pilots_online}
					</span>
				{/if}
			</span>

			<span class="flex items-center gap-2">
				<span class="font-mono text-[10px] whitespace-nowrap text-muted-foreground/60">
					{map.last_activity ? timeAgo(map.last_activity, now) : 'untouched'}
				</span>
				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="ghost"
								size="icon"
								class="size-5"
								aria-label="Actions for {map.name}"
								data-testid="map-menu"
							>
								<MoreVerticalIcon />
							</Button>
						{/snippet}
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="end">
						<DropdownMenu.Group>
							<DropdownMenu.Item onSelect={() => goto(`/maps/${map.id}/settings`)}>
								<SettingsIcon />
								Settings
							</DropdownMenu.Item>
							<DropdownMenu.Item
								onSelect={() => setPinned(map, !map.is_pinned)}
								data-testid="map-pin"
							>
								{#if map.is_pinned}
									<PinOffIcon />
									Unpin from the top bar
								{:else}
									<PinIcon />
									Pin to the top bar
								{/if}
							</DropdownMenu.Item>
							<DropdownMenu.Item
								onSelect={() => setArchived(map, !map.is_archived)}
								data-testid="map-archive"
							>
								{#if map.is_archived}
									<ArchiveRestoreIcon />
									Unarchive
								{:else}
									<ArchiveIcon />
									Archive
								{/if}
							</DropdownMenu.Item>
							{#if map.role === 'owner'}
								<DropdownMenu.Separator />
								<DropdownMenu.Item
									class="text-destructive data-highlighted:text-destructive"
									onSelect={() => (deleting = map)}
									data-testid="map-delete"
								>
									<Trash2Icon />
									Delete
								</DropdownMenu.Item>
							{/if}
						</DropdownMenu.Group>
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			</span>
		</div>
	</div>
{/snippet}

<div class="flex flex-col gap-4 p-6">
	<div class="flex flex-wrap items-end justify-between gap-3">
		<div>
			<h1 class="font-heading text-lg font-semibold tracking-tight">Maps</h1>
			<p class="mt-0.5 text-sm text-muted-foreground">
				Every chain you can reach, most recently flown first.
			</p>
		</div>
		<Button onclick={() => (creating = true)} data-testid="new-map">
			<PlusIcon data-icon="inline-start" />
			New map
		</Button>
	</div>

	{#if error}
		<p class="text-sm text-destructive" data-testid="maps-error">{error}</p>
	{/if}

	{#if maps.length > 0}
		<div class="flex w-fit divide-x divide-border border border-border">
			{#each [{ label: 'Maps', value: totals.maps }, { label: 'Systems', value: totals.systems }, { label: 'Pilots online', value: totals.pilots }] as stat (stat.label)}
				<div class="min-w-28 px-4 py-2">
					<div class="font-mono text-lg tabular-nums">{stat.value}</div>
					<div class="font-mono text-[10px] tracking-wider text-muted-foreground uppercase">
						{stat.label}
					</div>
				</div>
			{/each}
		</div>

		<div class="flex flex-wrap items-center gap-2">
			<div class="relative max-w-md flex-1">
				<SearchIcon class="absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
				<Input bind:value={query} placeholder="Search maps" class="pl-9" data-testid="map-search" />
			</div>
			{#if archived.length > 0}
				<Button
					variant="outline"
					onclick={() => (showArchived = !showArchived)}
					data-testid="toggle-archived"
				>
					{showArchived ? 'Hide' : 'Show'} archived ({archived.length})
				</Button>
			{/if}
		</div>
	{/if}

	{#if maps.length === 0}
		<div class="flex flex-col items-start gap-3 border border-dashed border-border p-8">
			<p class="text-sm text-muted-foreground" data-testid="maps-empty">
				No maps yet. A map is one chain: create one and start scanning.
			</p>
			<Button onclick={() => (creating = true)}>
				<PlusIcon data-icon="inline-start" />
				Create your first map
			</Button>
		</div>
	{:else if active.length === 0}
		<!-- Told apart from having no maps at all, because the fix is different. -->
		<div class="flex flex-col items-start gap-3 border border-dashed border-border p-8">
			<p class="text-sm text-muted-foreground" data-testid="maps-none-active">
				{query.trim()
					? `No active maps match "${query.trim()}".`
					: 'Every map you can reach is archived.'}
			</p>
			{#if archived.length > 0 && !showArchived}
				<Button variant="outline" onclick={() => (showArchived = true)}>
					Show archived ({archived.length})
				</Button>
			{/if}
		</div>
	{:else}
		<div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr))">
			{#each active as map (map.id)}
				{@render card(map)}
			{/each}
		</div>
	{/if}

	{#if showArchived && archived.length > 0}
		<div class="flex items-center gap-3 pt-2">
			<span class="font-mono text-[10px] tracking-wider text-muted-foreground uppercase">
				Archived · {archived.length}
			</span>
			<span class="h-px flex-1 bg-border"></span>
		</div>
		<div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr))">
			{#each archived as map (map.id)}
				{@render card(map)}
			{/each}
		</div>
	{/if}
</div>

<Dialog.Root bind:open={creating}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Create a map</Dialog.Title>
			<Dialog.Description>
				One map is one chain. Access and naming can be set up afterwards.
			</Dialog.Description>
		</Dialog.Header>
		<form
			onsubmit={(e) => {
				e.preventDefault();
				create();
			}}
		>
			<Field.FieldGroup>
				<Field.Field>
					<Field.FieldLabel for="map-name">Name</Field.FieldLabel>
					<Input
						id="map-name"
						bind:value={newName}
						placeholder="Home chain"
						autofocus
						data-testid="new-map-name"
					/>
				</Field.Field>
				<Field.Field>
					<Field.FieldLabel for="map-description">Description</Field.FieldLabel>
					<Textarea
						id="map-description"
						bind:value={newDescription}
						placeholder="Optional. What this chain is for."
						rows={2}
						data-testid="new-map-description"
					/>
				</Field.Field>
			</Field.FieldGroup>
			<Dialog.Footer class="mt-4">
				<Button type="button" variant="ghost" onclick={() => (creating = false)}>Cancel</Button>
				<Button type="submit" disabled={!newName.trim() || busy} data-testid="new-map-create">
					Create
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root open={deleting !== null} onOpenChange={(o) => !o && (deleting = null)}>
	<Dialog.Content class="sm:max-w-md" data-testid="delete-map-dialog">
		<Dialog.Header>
			<Dialog.Title>Delete “{deleting?.name}”?</Dialog.Title>
			<Dialog.Description>
				This removes the map and everything on it for everyone who can see it. There is no undo.
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="ghost" onclick={() => (deleting = null)}>Cancel</Button>
			<Button variant="destructive" onclick={confirmDelete} data-testid="confirm-delete-map">
				Delete map
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
