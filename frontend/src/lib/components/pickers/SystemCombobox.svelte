<script lang="ts">
	// A compact solar-system combobox: trigger shows the chosen system, opens a
	// Command-driven search (arrow keys + Enter to pick, Escape to close), rendering
	// the shared class/name/region rows.
	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import SystemRow from './SystemRow.svelte';

	let {
		placeholder,
		value,
		onpick
	}: {
		placeholder: string;
		value: number | null;
		onpick: (id: number | null) => void;
	} = $props();

	let open = $state(false);
	let query = $state('');
	let results = $state<SystemSearchResult[]>([]);
	// Monotonic request id: drop responses that arrive out of order while typing.
	let generation = 0;
	let label = $state('');
	let searchTimer: ReturnType<typeof setTimeout> | undefined;

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
			<Command.List class="max-h-48">
				<Command.Empty>
					{query.trim().length < 2
						? 'Type at least two characters to search.'
						: 'No systems found.'}
				</Command.Empty>
				<Command.Group>
					{#each results as s (s.id)}
						<Command.Item value={String(s.id)} onSelect={() => choose(s)}>
							<SystemRow system={s} />
						</Command.Item>
					{/each}
				</Command.Group>
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
