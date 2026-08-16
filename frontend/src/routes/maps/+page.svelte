<script lang="ts">
	import { api } from '$lib/api/client';
	import type { MapEntry } from '$lib/api/types/MapEntry';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';

	let maps = $state<MapEntry[] | null>(null);
	let newName = $state('');
	let error = $state('');

	$effect(() => {
		reload();
	});

	function reload() {
		api
			.myMaps()
			.then((list) => (maps = list))
			.catch((err) => (error = err.message));
	}

	async function create() {
		const name = newName.trim();
		if (!name) return;
		try {
			await api.createMap(name);
			newName = '';
			error = '';
			reload();
		} catch (err) {
			error = (err as Error).message;
		}
	}

	async function remove(id: number) {
		try {
			await api.deleteMap(id);
			reload();
		} catch (err) {
			error = (err as Error).message;
		}
	}
</script>

<div class="max-w-2xl">
	<h1 class="font-heading text-lg font-semibold tracking-tight">Your maps</h1>
	<p class="mt-1 h-4 text-sm text-destructive">{error}</p>

	<div class="mt-4 flex gap-2">
		<Input
			class="flex-1"
			placeholder="New map name"
			bind:value={newName}
			onkeydown={(ev) => ev.key === 'Enter' && create()}
		/>
		<Button onclick={create}>Create</Button>
	</div>

	{#if maps === null}
		<p class="mt-6 text-sm text-muted-foreground">Loading…</p>
	{:else if maps.length === 0}
		<p class="mt-6 text-sm text-muted-foreground">No maps yet.</p>
	{:else}
		<ul class="mt-6 divide-y divide-border border-y border-border">
			{#each maps as m (m.id)}
				<li class="group flex items-center justify-between py-2.5">
					<a
						href="/maps/{m.id}"
						class="text-sm text-foreground transition-colors hover:text-muted-foreground"
					>
						{m.name}
					</a>
					<span class="flex items-center gap-4">
						<Badge variant="outline" class="uppercase">{m.role}</Badge>
						<Button
							variant="ghost"
							size="sm"
							class="opacity-0 hover:text-destructive group-hover:opacity-100"
							onclick={() => remove(m.id)}
						>
							Delete
						</Button>
					</span>
				</li>
			{/each}
		</ul>
	{/if}
</div>
