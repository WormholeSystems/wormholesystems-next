<script lang="ts">
	// A compact solar-system combobox over a Command-driven search. Before anything is typed it
	// offers `suggestions` rather than an empty box, since the system you want is nearly always
	// one already in play (selected, where you are, a pinned watchlist entry).
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import XIcon from '@lucide/svelte/icons/x';
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import PinIcon from '@lucide/svelte/icons/pin';

	import { createQuery, keepPreviousData } from '@tanstack/svelte-query';

	import { q } from '$lib/api/queries';
	import { systemResolver } from '$lib/resolve-cache.svelte';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import { debounced } from '$lib/debounced.svelte';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import ClassBadge from '$lib/components/ClassBadge.svelte';
	import SystemRow from './SystemRow.svelte';

	let {
		placeholder,
		value,
		suggestions = [],
		onpick,
	}: {
		placeholder: string;
		value: number | null;
		/** Offered before a query is typed, each with the reason it is being offered. */
		suggestions?: {
			system: SystemSearchResult;
			reason: string;
			icon?: 'selected' | 'location' | 'pinned';
		}[];
		onpick: (id: number | null) => void;
	} = $props();

	let open = $state(false);
	let query = $state('');
	let label = $state('');

	/** Suggestions stand in for results until the query is long enough to search on. */
	const searching = $derived(query.trim().length >= 2);
	const offered = $derived(suggestions.filter((s) => s.system.id !== value));

	const settled = debounced(() => query.trim(), 150);
	// Keyed by term, so a slow reply can never land on a newer search.
	const search = createQuery(() => ({
		...q.searchSystems(settled.current),
		enabled: open && settled.current.length >= 2,
		placeholderData: keepPreviousData,
	}));
	const results = $derived(searching ? (search.data ?? []) : []);

	$effect(() => {
		if (value === null) {
			label = '';
			return;
		}
		systemResolver.resolve(value).then((hit) => (label = hit?.name ?? String(value)));
	});

	$effect(() => {
		if (open) query = '';
	});

	// See `pickers/columns.ts`: the list owns the tracks and rows are subgrids. The trailing
	// track is Command's own check indicator, which it appends to every item.
	const LIST_TRACKS =
		'grid grid-cols-[min-content_minmax(0,1fr)_minmax(0,0.8fr)_min-content_min-content] items-center gap-x-2';
	const ROW = 'col-span-full grid grid-cols-subgrid items-center gap-x-2';
	const CELLS = 'col-span-4 grid grid-cols-subgrid items-center gap-x-2';

	function choose(s: SystemSearchResult) {
		onpick(s.id);
		open = false;
	}
</script>

<div class="relative flex min-w-0 flex-1 items-center">
	<Popover.Root bind:open>
		<Popover.Trigger
			class="min-w-0 flex-1 border border-input bg-input/20 py-1 pl-2 text-left text-xs {value ===
			null
				? 'pr-2'
				: 'pr-6'} {label ? '' : 'text-muted-foreground'}"
			data-testid="system-picker-{placeholder.toLowerCase()}"
		>
			<span class="block truncate">{label || placeholder}</span>
		</Popover.Trigger>
		<Popover.Content class="w-96 p-0">
			<Command.Root shouldFilter={false}>
				<Command.Input placeholder="Search…" bind:value={query} />
				{#if !searching && offered.length > 0}
					<!-- Chips, not rows, so shortcuts do not read as search results. -->
					<div
						class="flex flex-wrap gap-1.5 border-b border-border/50 p-2"
						data-testid="picker-suggestions"
					>
						{#each offered as s (s.system.id)}
							<button
								type="button"
								class="inline-flex items-center gap-1.5 rounded-md border border-border/40 bg-muted/30 px-2 py-1 text-xs transition-colors hover:bg-muted/60"
								data-testid="picker-suggestion"
								title={s.reason}
								onclick={() => choose(s.system)}
							>
								<ClassBadge classId={s.system.wormhole_class_id} security={s.system.security} />
								{s.system.name}
								{#if s.icon === 'selected'}
									<MapPinIcon class="size-3 text-muted-foreground" />
								{:else if s.icon === 'location'}
									<NavigationIcon class="size-3 text-muted-foreground" />
								{:else if s.icon === 'pinned'}
									<PinIcon class="size-3 text-muted-foreground" />
								{/if}
							</button>
						{/each}
					</div>
				{/if}
				<Command.List class="max-h-64 p-1 {LIST_TRACKS}">
					{#if searching}
						<Command.Empty class="col-span-full">No systems found.</Command.Empty>
						{#each results as s (s.id)}
							<Command.Item
								value={String(s.id)}
								onSelect={() => choose(s)}
								class={ROW}
								data-testid="picker-result"
							>
								<SystemMenu system={s} class={CELLS}>
									<SystemRow system={s} />
								</SystemMenu>
							</Command.Item>
						{/each}
					{:else if offered.length === 0}
						<p class="col-span-full py-6 text-center text-sm text-muted-foreground">
							Type at least two characters to search.
						</p>
					{/if}
				</Command.List>
			</Command.Root>
		</Popover.Content>
	</Popover.Root>
	{#if value !== null}
		<!-- Unsetting matters as much as setting: an origin left behind keeps every distance
		     on the page measured from somewhere the pilot no longer is. -->
		<button
			type="button"
			class="absolute right-1 text-muted-foreground/60 hover:text-foreground"
			aria-label="Clear {placeholder.toLowerCase()}"
			data-testid="clear-{placeholder.toLowerCase()}"
			onclick={() => onpick(null)}
		>
			<XIcon class="size-3" />
		</button>
	{/if}
</div>
