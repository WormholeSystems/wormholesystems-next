<script lang="ts">
	// Somebody watching a chain they were handed a link to.
	//
	// Deliberately not the map page in a read-only costume: a watcher has no panels to
	// arrange, no settings, no socket and no pilots, and a page that hid all of that would
	// be mostly hiding. What is here is the chain and how to read it.
	import EyeIcon from '@lucide/svelte/icons/eye';

	import { api } from '$lib/api/client';
	import ReadOnlyMap from '$lib/components/map/ReadOnlyMap.svelte';
	import { timeAgo } from '$lib/format';

	let { data } = $props();

	// Seeded from the server load, then kept current by the poll below. `$state.raw`
	// because these are replaced wholesale rather than edited in place.
	let fetched = $state.raw<{ view: typeof data.view; signatures: typeof data.signatures } | null>(
		null
	);
	const view = $derived(fetched?.view ?? data.view);
	const signatures = $derived(fetched?.signatures ?? data.signatures);
	let refreshedAt = $state(new Date().toISOString());

	// A guest has no socket, so the page asks again on a timer. Slow enough to be polite,
	// often enough that a chain being flown stays worth watching.
	$effect(() => {
		const timer = setInterval(async () => {
			try {
				const [next, sigs] = await Promise.all([
					api.sharedMap(data.token),
					api.listSignatures(view.map.id, data.token)
				]);
				fetched = { view: next, signatures: sigs };
				refreshedAt = new Date().toISOString();
			} catch {
				// A withdrawn link stops updating rather than emptying the page.
			}
		}, 15_000);
		return () => clearInterval(timer);
	});
</script>

<svelte:head>
	<title>{view.map.name} · Vector</title>
</svelte:head>

<div class="flex h-[calc(100vh-3rem)] flex-col">
	<div class="flex items-center gap-3 border-b border-border px-4 py-2">
		<span class="font-heading text-sm font-semibold">{view.map.name}</span>
		{#if view.map.description}
			<span class="truncate text-xs text-muted-foreground">{view.map.description}</span>
		{/if}
		<span class="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground">
			<EyeIcon class="size-3.5" />
			Watching · updated {timeAgo(refreshedAt)}
		</span>
	</div>
	<div class="min-h-0 flex-1">
		<ReadOnlyMap {view} {signatures} />
	</div>
</div>
