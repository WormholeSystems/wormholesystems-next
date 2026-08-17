<script lang="ts">
	// A compact solar-system combobox: trigger shows the chosen system, opens a
	// Command-driven search (arrow keys + Enter to pick, Escape to close), rendering
	// the shared class/name/region rows.
	//
	// Before anything is typed it offers `suggestions` rather than an empty box. The systems
	// worth picking are nearly always ones already in play (the selected system, where you
	// are, a pinned watchlist entry), so the common case needs no typing at all.
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import NavigationIcon from '@lucide/svelte/icons/navigation';
	import PinIcon from '@lucide/svelte/icons/pin';

	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import SystemMenu from '$lib/components/system-menu/SystemMenu.svelte';
	import { classMeta } from '$lib/map/classes';
	import SystemRow from './SystemRow.svelte';

	let {
		placeholder,
		value,
		suggestions = [],
		onpick
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
	let results = $state<SystemSearchResult[]>([]);
	// Monotonic request id: drop responses that arrive out of order while typing.
	let generation = 0;
	let label = $state('');
	let searchTimer: ReturnType<typeof setTimeout> | undefined;

	/** Suggestions stand in for results until the query is long enough to search on. */
	const searching = $derived(query.trim().length >= 2);
	const offered = $derived(suggestions.filter((s) => s.system.id !== value));

	$effect(() => {
		if (value === null) {
			label = '';
			return;
		}
		api
			.resolveSystems([value])
			.then((rows) => (label = rows[0]?.name ?? String(value)))
			.catch(() => (label = String(value)));
	});

	$effect(() => {
		if (open) {
			query = '';
			results = [];
		}
	});

	$effect(() => {
		const text = query;
		clearTimeout(searchTimer);
		searchTimer = setTimeout(() => {
			const request = ++generation;
			api
				.searchSystems(text)
				.then((found) => {
					if (generation === request) results = found;
				})
				.catch(() => {});
		}, 150);
	});

	const ROW_TRACKS =
		'grid w-full grid-cols-[min-content_minmax(0,1fr)_minmax(0,0.8fr)_min-content] items-center gap-x-2';

	function choose(s: SystemSearchResult) {
		onpick(s.id);
		open = false;
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger
		class="min-w-0 flex-1 border border-input bg-input/20 px-2 py-1 text-left text-xs {label
			? ''
			: 'text-muted-foreground'}"
		data-testid="system-picker-{placeholder.toLowerCase()}"
	>
		<span class="block truncate">{label || placeholder}</span>
	</Popover.Trigger>
	<Popover.Content class="w-96 p-0">
		<Command.Root shouldFilter={false}>
			<Command.Input placeholder="Search…" bind:value={query} />
			{#if !searching && offered.length > 0}
				<!-- Chips rather than rows: these are shortcuts to systems already in play, and
				     drawing them like search results would suggest they are search results. -->
				<div class="flex flex-wrap gap-1.5 border-b border-border/50 p-2" data-testid="picker-suggestions">
					{#each offered as s (s.system.id)}
						<button
							type="button"
							class="inline-flex items-center gap-1.5 rounded-md border border-border/40 bg-muted/30 px-2 py-1 text-xs transition-colors hover:bg-muted/60"
							data-testid="picker-suggestion"
							title={s.reason}
							onclick={() => choose(s.system)}
						>
							<span class="font-mono {classMeta(s.system.wormhole_class_id, s.system.security).token}">
								{classMeta(s.system.wormhole_class_id, s.system.security).short}
							</span>
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
			<Command.List class="max-h-64">
				{#if searching}
					<Command.Empty>No systems found.</Command.Empty>
					<Command.Group>
						{#each results as s (s.id)}
							<Command.Item
								value={String(s.id)}
								onSelect={() => choose(s)}
								data-testid="picker-result"
							>
								<SystemMenu system={s} class={ROW_TRACKS}>
									<SystemRow system={s} />
								</SystemMenu>
							</Command.Item>
						{/each}
					</Command.Group>
				{:else if offered.length === 0}
					<p class="py-6 text-center text-sm text-muted-foreground">
						Type at least two characters to search.
					</p>
				{/if}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
