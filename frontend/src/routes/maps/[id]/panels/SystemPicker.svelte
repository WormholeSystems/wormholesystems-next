<script lang="ts">
	// A compact system picker: shows the chosen system, opens a search popover.
	import { api } from '$lib/api/client';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import { Input } from '$lib/components/ui/input';
	import * as Popover from '$lib/components/ui/popover';

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
	let generation = 0;
	let label = $state('');

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

	function runSearch(text: string) {
		query = text;
		const request = ++generation;
		api
			.searchSystems(text)
			.then((found) => {
				if (generation === request) results = found;
			})
			.catch(() => {});
	}

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
	<Popover.Content class="flex w-56 flex-col gap-1 p-1">
		<Input
			placeholder="Search…"
			value={query}
			oninput={(ev) => runSearch(ev.currentTarget.value)}
		/>
		<div class="flex max-h-48 flex-col overflow-auto">
			{#each results as s (s.id)}
				<button
					class="flex items-center gap-2 px-2 py-1 text-left text-xs hover:bg-accent"
					onclick={() => choose(s)}
				>
					<span class="truncate">{s.name}</span>
					<span class="ml-auto truncate text-muted-foreground">{s.region}</span>
				</button>
			{/each}
		</div>
	</Popover.Content>
</Popover.Root>
