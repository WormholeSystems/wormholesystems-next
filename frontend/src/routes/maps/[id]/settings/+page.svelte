<script lang="ts">
	// Map settings: the name, and who can see the map. Access is the interesting half —
	// a grant can target a character, their corporation, or their alliance, and the
	// server refuses anything that would leave the map without an owner.
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	import { api } from '$lib/api/client';
	import type { AccessEntry } from '$lib/api/types/AccessEntry';
	import type { AccessSubject } from '$lib/api/types/AccessSubject';
	import type { MapView } from '$lib/api/types/MapView';
	import type { Role } from '$lib/api/types/Role';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import EveImage from '$lib/components/EveImage.svelte';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import { cn } from '$lib/utils';

	const mapId = $derived(Number(page.params.id) || 0);

	let view = $state<MapView | null>(null);
	let access = $state<AccessEntry[]>([]);
	let name = $state('');
	let error = $state('');

	// Grant form: a search over cached entities, or a raw EVE id typed straight in.
	let query = $state('');
	let matches = $state<AccessSubject[]>([]);
	let picked = $state<AccessSubject | null>(null);
	let newRole = $state<Role>('member');

	const canManage = $derived(view?.role === 'manager' || view?.role === 'owner');
	const isOwner = $derived(view?.role === 'owner');
	const ROLES: Role[] = ['viewer', 'member', 'manager', 'owner'];
	const ROLE_HELP: Record<Role, string> = {
		viewer: 'Can see the map, but change nothing.',
		member: 'Can add systems, connections and signatures.',
		manager: 'Can also grant and revoke access.',
		owner: 'Full control, including deleting the map.'
	};

	$effect(() => {
		if (mapId) reload();
	});

	async function reload() {
		try {
			const [v, a] = await Promise.all([api.fetchMap(mapId), api.listAccess(mapId)]);
			view = v;
			access = a;
			name = v.map.name;
		} catch (err) {
			error = (err as Error).message;
		}
	}

	$effect(() => {
		const q = query.trim();
		if (q.length < 2) {
			matches = [];
			return;
		}
		let cancelled = false;
		api
			.searchAccessSubjects(q)
			.then((r) => !cancelled && (matches = r))
			.catch(() => {});
		return () => (cancelled = true);
	});

	async function act(work: Promise<unknown>) {
		try {
			await work;
			error = '';
			await reload();
		} catch (err) {
			error = (err as Error).message;
		}
	}

	function rename() {
		const next = name.trim();
		if (!next || next === view?.map.name) return;
		act(api.updateMap({ map_id: mapId, name: next }));
	}

	function grant() {
		// A pasted id has no cached name; grant it as a character and let the next load
		// resolve whatever it turns out to be.
		const raw = Number(query.trim());
		const subject = picked ?? (raw > 0 ? { subject_type: 'character' as const, subject_id: raw } : null);
		if (!subject) return;
		act(
			api.setAccess({
				map_id: mapId,
				subject_type: subject.subject_type,
				subject_id: subject.subject_id,
				role: newRole
			})
		).then(() => {
			query = '';
			picked = null;
			matches = [];
		});
	}

	async function destroy() {
		if (!confirm(`Delete "${view?.map.name}"? This cannot be undone.`)) return;
		try {
			await api.deleteMap(mapId);
			goto('/maps');
		} catch (err) {
			error = (err as Error).message;
		}
	}
</script>

<div class="mx-auto flex max-w-2xl flex-col gap-6 py-6">
	<a
		href="/maps/{mapId}"
		class="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
	>
		<ArrowLeftIcon class="size-4" />
		Back to the map
	</a>

	{#if error}
		<p class="text-sm text-destructive" data-testid="settings-error">{error}</p>
	{/if}

	<Card.Root>
		<Card.Header>
			<Card.Title>Map</Card.Title>
			<Card.Description>The name everyone on this map sees.</Card.Description>
		</Card.Header>
		<Card.Content>
			<div class="flex flex-col gap-2">
				<label for="map-name" class="text-sm font-medium">Name</label>
				<div class="flex gap-2">
						<Input
							id="map-name"
							bind:value={name}
							disabled={!canManage}
							data-testid="map-name-input"
							onkeydown={(e) => e.key === 'Enter' && rename()}
						/>
					<Button
						variant="outline"
						disabled={!canManage || !name.trim() || name.trim() === view?.map.name}
						onclick={rename}
						data-testid="rename-button">Save</Button
					>
				</div>
			</div>
		</Card.Content>
	</Card.Root>

	<Card.Root>
		<Card.Header>
			<Card.Title>Access</Card.Title>
			<Card.Description>
				Granting a corporation or alliance covers every pilot in it.
			</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col gap-4">
			{#if canManage}
				<div class="flex flex-col gap-2">
					<label for="grant-search" class="text-sm font-medium">
						Add a character, corp or alliance
					</label>
					<div class="flex gap-2">
							<div class="relative flex-1">
								<Input
									id="grant-search"
									bind:value={query}
									placeholder="Name, ticker, or an EVE id"
									data-testid="grant-search"
									oninput={() => (picked = null)}
								/>
								{#if matches.length > 0 && !picked}
									<ul
										class="absolute z-10 mt-1 max-h-56 w-full overflow-y-auto rounded-md border border-border bg-popover py-1 shadow-md"
										data-testid="grant-matches"
									>
										{#each matches as m (m.subject_type + m.subject_id)}
											<li>
												<button
													type="button"
													class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-sm hover:bg-accent"
													onclick={() => {
														picked = m;
														query = m.name;
														matches = [];
													}}
												>
													<EveImage
										kind={m.subject_type}
										id={m.subject_id}
										size={64}
										title={m.name}
										class="size-6 rounded-sm"
									/>
													<span class="flex-1 truncate">{m.name}</span>
													{#if m.ticker}
														<span class="text-xs text-muted-foreground">[{m.ticker}]</span>
													{/if}
													<span class="text-xs text-muted-foreground">{m.subject_type}</span>
												</button>
											</li>
										{/each}
									</ul>
								{/if}
							</div>
							<Select.Root type="single" bind:value={newRole}>
								<Select.Trigger class="w-32" data-testid="grant-role">
									{newRole}
								</Select.Trigger>
								<Select.Content>
									<Select.Group>
										{#each ROLES as r (r)}
											<Select.Item value={r} label={r}>{r}</Select.Item>
										{/each}
									</Select.Group>
								</Select.Content>
							</Select.Root>
							<Button onclick={grant} disabled={!picked && !Number(query.trim())} data-testid="grant-button">
								Grant
							</Button>
						</div>
					<p class="text-xs text-muted-foreground">{ROLE_HELP[newRole]}</p>
				</div>
			{/if}

			<ul class="flex flex-col divide-y divide-border/50" data-testid="access-list">
				{#each access as entry (entry.subject_id)}
					<li class="flex items-center gap-3 py-2">
						<EveImage
							kind={entry.subject_type}
							id={entry.subject_id}
							size={64}
							title={entry.name ?? String(entry.subject_id)}
							class="size-8 rounded-sm"
						/>
						<span class="flex-1 truncate text-sm">
							{entry.name ?? `Unknown (${entry.subject_id})`}
							<span class="ml-1 text-xs text-muted-foreground">{entry.subject_type}</span>
						</span>
						{#if canManage}
							<Select.Root
								type="single"
								value={entry.role}
								onValueChange={(role) =>
									act(
										api.setAccess({
											map_id: mapId,
											subject_type: entry.subject_type,
											subject_id: entry.subject_id,
											role: role as Role
										})
									)}
							>
								<Select.Trigger class="w-28">{entry.role}</Select.Trigger>
								<Select.Content>
									<Select.Group>
										{#each ROLES as r (r)}
											<Select.Item value={r} label={r}>{r}</Select.Item>
										{/each}
									</Select.Group>
								</Select.Content>
							</Select.Root>
							<Button
								variant="ghost"
								size="icon"
								class="size-8 text-muted-foreground hover:text-destructive"
								onclick={() =>
									act(api.revokeAccess({ map_id: mapId, subject_id: entry.subject_id }))}
							>
								<TrashIcon />
							</Button>
						{:else}
							<Badge variant="outline">{entry.role}</Badge>
						{/if}
					</li>
				{/each}
			</ul>
		</Card.Content>
	</Card.Root>

	{#if isOwner}
		<Card.Root class={cn('border-destructive/40')}>
			<Card.Header>
				<Card.Title>Delete this map</Card.Title>
				<Card.Description>
					Removes the map and everything on it for everyone. There is no undo.
				</Card.Description>
			</Card.Header>
			<Card.Footer>
				<Button variant="destructive" onclick={destroy} data-testid="delete-map">
					<TrashIcon data-icon="inline-start" />
					Delete map
				</Button>
			</Card.Footer>
		</Card.Root>
	{/if}
</div>
