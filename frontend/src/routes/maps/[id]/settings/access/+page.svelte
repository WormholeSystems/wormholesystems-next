<script lang="ts">
	// Who can see the map, and what they may do on it. A grant can target a character,
	// their corporation or their alliance, and the server refuses anything that would leave
	// the map without an owner.
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { AccessEntry } from '$lib/api/types/AccessEntry';
	import type { AccessSubject } from '$lib/api/types/AccessSubject';
	import type { MapView } from '$lib/api/types/MapView';
	import type { Role } from '$lib/api/types/Role';
	import EveImage from '$lib/components/EveImage.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import * as Select from '$lib/components/ui/select';

	const mapId = $derived(Number(page.params.id) || 0);

	let view = $state<MapView | null>(null);
	let access = $state<AccessEntry[]>([]);
	let error = $state('');

	// Grant form: a search over cached entities, or a raw EVE id typed straight in.
	let query = $state('');
	let matches = $state<AccessSubject[]>([]);
	let picked = $state<AccessSubject | null>(null);
	let picking = $state(false);
	let newRole = $state<Role>('member');

	const canManage = $derived(view?.role === 'manager' || view?.role === 'owner');
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

	function choose(subject: AccessSubject) {
		picked = subject;
		picking = false;
	}

	async function act(work: Promise<unknown>) {
		try {
			await work;
			error = '';
			await reload();
		} catch (err) {
			error = (err as Error).message;
		}
	}

	// How long a grant lasts. Access for one operation is the common case people forget to
	// tidy up afterwards, so the form can hand it an end from the start.
	const DURATIONS = [
		{ value: 'forever', label: 'No end date', hours: null },
		{ value: '12h', label: 'For 12 hours', hours: 12 },
		{ value: '24h', label: 'For a day', hours: 24 },
		{ value: '7d', label: 'For a week', hours: 24 * 7 },
		{ value: '30d', label: 'For a month', hours: 24 * 30 }
	];
	let newDuration = $state('forever');

	function endsAt(duration: string): string | null {
		const hours = DURATIONS.find((d) => d.value === duration)?.hours ?? null;
		return hours === null ? null : new Date(Date.now() + hours * 3600_000).toISOString();
	}

	function grant() {
		// A pasted id has no cached name; grant it as a character and let the next load
		// resolve whatever it turns out to be.
		const raw = Number(query.trim());
		const subject =
			picked ?? (raw > 0 ? { subject_type: 'character' as const, subject_id: raw } : null);
		if (!subject) return;
		act(
			api.setAccess({
				map_id: mapId,
				subject_type: subject.subject_type,
				subject_id: subject.subject_id,
				role: newRole,
				expires_at: endsAt(newDuration)
			})
		).then(() => {
			query = '';
			picked = null;
			matches = [];
			newDuration = 'forever';
		});
	}
</script>

{#if error}
	<p class="mb-4 text-sm text-destructive" data-testid="settings-error">{error}</p>
{/if}

<Card.Root>
	<Card.Header>
		<Card.Title>Access</Card.Title>
		<Card.Description>Granting a corporation or alliance covers every pilot in it.</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-col gap-4">
		{#if canManage}
			<div class="flex flex-col gap-2">
				<label for="grant-search" class="text-sm font-medium">
					Add a character, corp or alliance
				</label>
				<div class="flex gap-2">
						<Popover.Root bind:open={picking}>
							<Popover.Trigger
								class="min-w-0 flex-1 border border-input bg-input/20 px-3 py-1.5 text-left text-sm {picked
									? ''
									: 'text-muted-foreground'}"
								data-testid="grant-search"
							>
								<span class="block truncate">
									{picked ? picked.name : 'Name, ticker, or an EVE id'}
								</span>
							</Popover.Trigger>
							<Popover.Content class="w-96 p-0" align="start">
								<!-- Matching happens server-side, so Command's own filter stays out of it. -->
								<Command.Root shouldFilter={false}>
									<Command.Input placeholder="Name, ticker, or an EVE id…" bind:value={query} />
									<Command.List data-testid="grant-matches">
										<Command.Empty>
											{query.trim().length < 2
												? 'Type at least two characters.'
												: 'Nothing found. An EVE id works too.'}
										</Command.Empty>
										<Command.Group>
											{#each matches as m (m.subject_type + m.subject_id)}
												<Command.Item
													value={`${m.subject_type}-${m.subject_id}`}
													onSelect={() => choose(m)}
													data-testid="grant-match"
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
												</Command.Item>
											{/each}
										</Command.Group>
									</Command.List>
								</Command.Root>
							</Popover.Content>
						</Popover.Root>
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
						<Select.Root type="single" value={newDuration} onValueChange={(v) => v && (newDuration = v)}>
							<Select.Trigger class="w-36" data-testid="grant-duration">
								{DURATIONS.find((d) => d.value === newDuration)?.label}
							</Select.Trigger>
							<Select.Content>
								<Select.Group>
									{#each DURATIONS as option (option.value)}
										<Select.Item value={option.value} label={option.label}>
											{option.label}
										</Select.Item>
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
					<span class="flex min-w-0 flex-1 flex-col">
						<span class="truncate text-sm">
							{entry.name ?? `Unknown (${entry.subject_id})`}
							<span class="ml-1 text-xs text-muted-foreground">{entry.subject_type}</span>
						</span>
						{#if entry.expires_at}
							<button
								type="button"
								class="w-fit text-xs text-amber-500 hover:underline"
								data-testid="access-expiry"
								title={canManage ? 'Click to make it permanent' : undefined}
								disabled={!canManage}
								onclick={() =>
									act(
										api.setAccess({
											map_id: mapId,
											subject_type: entry.subject_type,
											subject_id: entry.subject_id,
											role: entry.role,
											expires_at: null
										})
									)}
							>
								Until {new Date(entry.expires_at).toLocaleString()}
							</button>
						{/if}
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
