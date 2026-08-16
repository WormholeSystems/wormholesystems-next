<script lang="ts">
	// Per-system notes (markdown). Member-gated: viewers get a 403 from the details
	// endpoint and see nothing. Pencil to edit, plain textarea, explicit save.
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import { marked } from 'marked';

	import { api, ApiError } from '$lib/api/client';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Textarea } from '$lib/components/ui/textarea';
	import type { MapState } from '../map-state.svelte';

	let { map, system }: { map: MapState; system: MapSystemView } = $props();

	let notes = $state<string | null>(null);
	let hidden = $state(false);
	let editing = $state(false);
	let draft = $state('');

	$effect(() => {
		const mss = system.id;
		editing = false;
		notes = null;
		hidden = false;
		api
			.systemDetails(map.mapId, mss)
			.then((d) => (notes = d.notes))
			.catch((err) => {
				if (err instanceof ApiError && (err.status === 403 || err.status === 404)) hidden = true;
			});
	});

	const rendered = $derived.by(() => {
		if (!notes) return '';
		const html = marked.parse(notes, { async: false, breaks: true }) as string;
		// Open links in a new tab.
		return html.replaceAll('<a href', '<a target="_blank" rel="noopener" href');
	});

	function startEdit() {
		draft = notes ?? '';
		editing = true;
	}

	function save() {
		const value = draft.trim() || null;
		map.run(
			'notes',
			api.setNotes({ map_id: map.mapId, map_solar_system_id: system.id, notes: value })
		);
		notes = value;
		editing = false;
	}
</script>

{#if !hidden}
	<Card.Root data-testid="notes-card">
		<Card.Header>
			<Card.Title class="flex items-center justify-between">
				Notes
				{#if !editing}
					<Button variant="ghost" size="icon-xs" aria-label="Edit notes" onclick={startEdit}>
						<PencilIcon />
					</Button>
				{:else}
					<span class="flex gap-1">
						<Button variant="ghost" size="xs" onclick={() => (editing = false)}>Cancel</Button>
						<Button size="xs" onclick={save}>Save</Button>
					</span>
				{/if}
			</Card.Title>
		</Card.Header>
		<Card.Content class="text-xs">
			{#if editing}
				<Textarea class="min-h-28 font-mono text-xs" bind:value={draft} placeholder="Notes (markdown)…" />
			{:else if rendered}
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				<div class="prose prose-invert prose-xs max-w-none [&_a]:underline">{@html rendered}</div>
			{:else}
				<p class="text-muted-foreground">No notes</p>
			{/if}
		</Card.Content>
	</Card.Root>
{/if}
