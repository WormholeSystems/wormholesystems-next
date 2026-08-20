<script lang="ts">
	// Who can see the map, and what they may do on it. A grant can target a character,
	// their corporation or their alliance, and the server refuses anything that would leave
	// the map without an owner.
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { invalidate } from '$app/navigation';
	import { page } from '$app/state';
	import { toast } from 'svelte-sonner';

	import { api } from '$lib/api/client';
	import type { AccessEntry } from '$lib/api/types/AccessEntry';
	import type { AccessSubject } from '$lib/api/types/AccessSubject';
	import type { MapView } from '$lib/api/types/MapView';
	import type { Role } from '$lib/api/types/Role';
	import EveImage from '$lib/components/EveImage.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import { getLocalTimeZone, today, type DateValue } from '@internationalized/date';

	import { Calendar } from '$lib/components/ui/calendar';
	import ArrowIcon from '@lucide/svelte/icons/arrow-up';

	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import * as Command from '$lib/components/ui/command';
	import { Input } from '$lib/components/ui/input';
	import * as Table from '$lib/components/ui/table';
	import { Switch } from '$lib/components/ui/switch';
	import SettingRow from '$lib/components/settings/SettingRow.svelte';
	import * as Popover from '$lib/components/ui/popover';
	import * as Select from '$lib/components/ui/select';
	import { atLeast, byRole, ROLE_HELP, ROLE_LABEL } from '$lib/map/roles';

	let { data }: { data: { view: MapView; access: AccessEntry[] } } = $props();

	const mapId = $derived(Number(page.params.id) || 0);
	const view = $derived(data.view);
	const access = $derived(data.access);
	let error = $state('');

	let query = $state('');
	let matches = $state<AccessSubject[]>([]);
	let picked = $state<AccessSubject | null>(null);
	let picking = $state(false);
	let newRole = $state<Role>('member');

	const canManage = $derived(atLeast(view.role, 'manager'));
	// Ownership is not grantable here: it is handed on from the General section instead.
	const ROLES: Role[] = ['viewer', 'member', 'manager'];
	const ALL_ROLES: Role[] = ['viewer', 'member', 'manager', 'owner'];

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

	let filter = $state('');
	let sort = $state<{ key: 'name' | 'subject_type' | 'role' | 'expires_at'; descending: boolean }>({
		key: 'role',
		descending: false
	});
	const COLUMNS = [
		{ key: 'name' as const, label: 'Who' },
		{ key: 'subject_type' as const, label: 'Kind' },
		{ key: 'role' as const, label: 'Role' },
		{ key: 'expires_at' as const, label: 'Ends' }
	];

	function sortBy(key: (typeof COLUMNS)[number]['key']) {
		sort = sort.key === key ? { key, descending: !sort.descending } : { key, descending: false };
	}

	const shown = $derived.by(() => {
		const needle = filter.trim().toLowerCase();
		const rows = access.filter(
			(e) =>
				!needle ||
				(e.name ?? '').toLowerCase().includes(needle) ||
				String(e.subject_id).includes(needle)
		);
		const compare = (a: AccessEntry, b: AccessEntry) => {
			switch (sort.key) {
				case 'role':
					return byRole(a.role, b.role);
				case 'subject_type':
					return a.subject_type.localeCompare(b.subject_type);
				case 'expires_at':
					// The ones that end come first; permanent grants sort last.
					return (
						(a.expires_at ? Date.parse(a.expires_at) : Infinity) -
						(b.expires_at ? Date.parse(b.expires_at) : Infinity)
					);
				default:
					return (a.name ?? '').localeCompare(b.name ?? '');
			}
		};
		return [...rows].sort(
			(a, b) => (sort.descending ? -1 : 1) * (compare(a, b) || (a.name ?? '').localeCompare(b.name ?? ''))
		);
	});

	/** The grant whose end date is being dropped, once it has been confirmed. */
	let clearing = $state<AccessEntry | null>(null);

	function keepForever() {
		const entry = clearing;
		clearing = null;
		if (!entry) return;
		act(
			api.setAccess({
				map_id: mapId,
				subject_type: entry.subject_type,
				subject_id: entry.subject_id,
				role: entry.role,
				expires_at: null
			})
		);
	}

	// Only a manager sees the link: the API withholds the token from anyone else.
	let revoking = $state(false);
	const shareUrl = $derived(
		view.map.share_token ? `${page.url.origin}/share/${view.map.share_token}` : ''
	);

	function rotateShare() {
		act(api.shareMap(mapId)).then(() => toast.success('Share link ready'));
	}

	function revokeShare() {
		revoking = false;
		act(api.unshareMap(mapId)).then(() => toast.success('Share link withdrawn'));
	}

	async function copyShare() {
		try {
			await navigator.clipboard.writeText(shareUrl);
			toast.success('Share link copied');
		} catch {
			toast.error('Clipboard access denied');
		}
	}

	async function act(work: Promise<unknown>) {
		try {
			await work;
			error = '';
			await Promise.all([invalidate('vector:access'), invalidate('vector:map')]);
		} catch (err) {
			error = (err as Error).message;
		}
	}

	// Access for one operation is the case people forget to tidy up, so the form can set an
	// end from the start.
	const DURATIONS = [
		{ hours: 12, label: 'For 12 hours' },
		{ hours: 24, label: 'For a day' },
		{ hours: 24 * 7, label: 'For a week' },
		{ hours: 24 * 30, label: 'For a month' }
	];
	/** `null` is the permanent grant; a date ends at the close of that day. */
	let ends = $state<Date | null>(null);
	let picking_date = $state(false);
	let customDate = $state<DateValue | undefined>(undefined);

	const endsLabel = $derived(
		ends === null
			? 'No end date'
			: ends.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' })
	);

	function endsIn(hours: number) {
		ends = new Date(Date.now() + hours * 3600_000);
		customDate = undefined;
		picking_date = false;
	}

	function endsOn(date: DateValue | undefined) {
		if (!date) return;
		// The close of the chosen day, so "until the 24th" includes the 24th.
		const day = date.toDate(getLocalTimeZone());
		day.setHours(23, 59, 59, 0);
		ends = day;
		picking_date = false;
	}

	function grant() {
		// A pasted id has no cached name; grant it as a character and let the next load resolve
		// whatever it turns out to be.
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
				expires_at: ends?.toISOString() ?? null
			})
		).then(() => {
			query = '';
			picked = null;
			matches = [];
			ends = null;
			customDate = undefined;
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
				<div class="flex items-center gap-2">
						<Popover.Root bind:open={picking}>
							<Popover.Trigger
								class="flex h-7 min-w-0 flex-1 items-center rounded-md border border-input bg-input/20 px-2 text-left text-xs/relaxed {picked
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
								{ROLE_LABEL[newRole]}
							</Select.Trigger>
							<Select.Content>
								<Select.Group>
									{#each ROLES as r (r)}
										<Select.Item value={r} label={ROLE_LABEL[r]}>{ROLE_LABEL[r]}</Select.Item>
									{/each}
								</Select.Group>
							</Select.Content>
						</Select.Root>
						<Popover.Root bind:open={picking_date}>
							<Popover.Trigger
								class="flex h-7 w-40 items-center justify-between gap-1.5 rounded-md border border-input bg-input/20 px-2 text-xs/relaxed {ends
									? ''
									: 'text-muted-foreground'}"
								data-testid="grant-duration"
							>
								{endsLabel}
								<CalendarIcon class="size-3.5 shrink-0 text-muted-foreground" />
							</Popover.Trigger>
							<Popover.Content class="w-auto p-0" align="end">
								<div class="flex flex-col border-b border-border/50 p-1">
									<button
										class="px-2 py-1 text-left text-xs hover:bg-accent"
										data-testid="duration-forever"
										onclick={() => {
											ends = null;
											customDate = undefined;
											picking_date = false;
										}}
									>
										No end date
									</button>
									{#each DURATIONS as option (option.hours)}
										<button
											class="px-2 py-1 text-left text-xs hover:bg-accent"
											data-testid="duration-{option.hours}"
											onclick={() => endsIn(option.hours)}
										>
											{option.label}
										</button>
									{/each}
								</div>
								<Calendar
									type="single"
									bind:value={customDate}
									minValue={today(getLocalTimeZone())}
									onValueChange={endsOn}
									data-testid="duration-calendar"
								/>
							</Popover.Content>
						</Popover.Root>
						<Button
							onclick={grant}
							disabled={!picked && !Number(query.trim())}
							data-testid="grant-button"
						>
							Grant
						</Button>
					</div>
			</div>

			<!-- Owner is listed to be read, not chosen: it is handed on, not granted. -->
			<div class="border border-border/60" data-testid="role-help">
				{#each ALL_ROLES as r (r)}
					<div
						class="flex items-start gap-3 border-b border-border/40 px-3 py-2 last:border-b-0 {newRole ===
						r
							? 'bg-accent/30'
							: ''}"
					>
						<span class="w-20 shrink-0 text-xs font-medium">{ROLE_LABEL[r]}</span>
						<span class="text-xs leading-relaxed text-muted-foreground">{ROLE_HELP[r]}</span>
					</div>
				{/each}
			</div>
		{/if}

		<div class="flex items-center gap-2">
			<Input
				bind:value={filter}
				placeholder="Filter by name…"
				class="h-7 w-56 text-xs"
				data-testid="access-filter"
			/>
			<span class="text-xs text-muted-foreground">
				{shown.length} of {access.length}
			</span>
		</div>

		<Table.Root data-testid="access-list">
			<Table.Header>
				<Table.Row>
					{#each COLUMNS as column (column.key)}
						<Table.Head>
							<button
								class="flex items-center gap-1 hover:text-foreground"
								data-testid="sort-{column.key}"
								onclick={() => sortBy(column.key)}
							>
								{column.label}
								{#if sort.key === column.key}
									<ArrowIcon
										class="size-3 {sort.descending ? 'rotate-180' : ''} transition-transform"
									/>
								{/if}
							</button>
						</Table.Head>
					{/each}
					<Table.Head class="w-24"></Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each shown as entry (entry.subject_id)}
					<Table.Row data-testid="access-row">
						<Table.Cell>
							<span class="flex min-w-0 items-center gap-2">
								<EveImage
									kind={entry.subject_type}
									id={entry.subject_id}
									size={64}
									title={entry.name ?? String(entry.subject_id)}
									class="size-7 shrink-0 rounded-sm"
								/>
								<span class="truncate">{entry.name ?? `Unknown (${entry.subject_id})`}</span>
							</span>
						</Table.Cell>
						<Table.Cell class="text-muted-foreground">{entry.subject_type}</Table.Cell>
						<Table.Cell>
							{#if entry.role === 'owner'}
								<Badge variant="outline">{ROLE_LABEL.owner}</Badge>
							{:else if canManage}
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
									<Select.Trigger class="w-28">{ROLE_LABEL[entry.role]}</Select.Trigger>
									<Select.Content>
										<Select.Group>
											{#each ROLES as r (r)}
												<Select.Item value={r} label={ROLE_LABEL[r]}>{ROLE_LABEL[r]}</Select.Item>
											{/each}
										</Select.Group>
									</Select.Content>
								</Select.Root>
							{:else}
								<Badge variant="outline">{ROLE_LABEL[entry.role]}</Badge>
							{/if}
						</Table.Cell>
						<Table.Cell>
							{#if entry.expires_at}
								<button
									type="button"
									class="text-xs text-amber-500 hover:underline disabled:no-underline"
									data-testid="access-expiry"
									title={canManage ? 'Remove the end date' : undefined}
									disabled={!canManage}
									onclick={() => (clearing = entry)}
								>
									{new Date(entry.expires_at).toLocaleDateString(undefined, {
										day: 'numeric',
										month: 'short',
										year: 'numeric'
									})}
								</button>
							{:else}
								<span class="text-xs text-muted-foreground">—</span>
							{/if}
						</Table.Cell>
						<Table.Cell class="text-right">
							{#if canManage && entry.role !== 'owner'}
								<Button
									variant="ghost"
									size="icon"
									class="size-7 text-muted-foreground hover:text-destructive"
									aria-label="Revoke access for {entry.name ?? entry.subject_id}"
									onclick={() =>
										act(api.revokeAccess({ map_id: mapId, subject_id: entry.subject_id }))}
								>
									<TrashIcon />
								</Button>
							{/if}
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>

	</Card.Content>
</Card.Root>

<!-- Sharing lives here because it answers the same question as the grants (who can see the
     chain) for people without an account. -->
{#if canManage}
	<Card.Root class="mt-6" data-testid="sharing-card">
		<Card.Header>
			<Card.Title>Sharing</Card.Title>
			<Card.Description>
				Both open the map itself, read-only: the chain as it is scanned, with no editing
				and no pilots.
			</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col py-0">
			<SettingRow
				id="share-link"
				label="Share link"
				description="Anyone holding this address can watch the map without an account; following it opens the map itself. Making a new link locks out whoever had the old one."
			>
				{#snippet control()}
					<span class="flex items-center gap-2">
						{#if shareUrl}
							<Input
								readonly
								value={shareUrl}
								class="h-7 w-64 font-mono text-[11px]"
								data-testid="share-url"
								onfocus={(e) => e.currentTarget.select()}
							/>
							<Button variant="outline" onclick={copyShare} data-testid="share-copy">Copy</Button>
							<Button variant="ghost" onclick={rotateShare} title="Replace the link">New</Button>
							<Button
								variant="ghost"
								class="text-destructive hover:text-destructive"
								onclick={() => (revoking = true)}
								data-testid="share-revoke"
							>
								Revoke
							</Button>
						{:else}
							<Button variant="outline" onclick={rotateShare} data-testid="share-create">
								Create a link
							</Button>
						{/if}
					</span>
				{/snippet}
			</SettingRow>

			<SettingRow
				id="map-public"
				label="Public map"
				description="Anyone at all can watch it, with no link needed and nothing to guess. Turn this on for a map you would put on a forum post."
			>
				{#snippet control()}
					<Switch
						checked={view.map.is_public}
						aria-label="Public map"
						data-testid="share-public"
						onCheckedChange={(v) => act(api.updateMap({ map_id: mapId, is_public: v }))}
					/>
				{/snippet}
			</SettingRow>
		</Card.Content>
	</Card.Root>
{/if}

<AlertDialog.Root open={revoking} onOpenChange={(o) => (revoking = o)}>
	<AlertDialog.Content data-testid="revoke-share-dialog">
		<AlertDialog.Header>
			<AlertDialog.Title>Withdraw the share link?</AlertDialog.Title>
			<AlertDialog.Description>
				Everyone watching through it loses the map at once. People with a grant are not
				affected, and you can always make a new link.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Keep it</AlertDialog.Cancel>
			<AlertDialog.Action onclick={revokeShare} data-testid="revoke-share-confirm">
				Withdraw it
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>

<AlertDialog.Root open={clearing !== null} onOpenChange={(o) => !o && (clearing = null)}>
	<AlertDialog.Content data-testid="clear-expiry-dialog">
		<AlertDialog.Header>
			<AlertDialog.Title>Drop the end date?</AlertDialog.Title>
			<AlertDialog.Description>
				{clearing?.name ?? 'They'} keeps {ROLE_LABEL[clearing?.role ?? 'viewer'].toLowerCase()} access
				until somebody takes it away, instead of until
				{clearing?.expires_at ? new Date(clearing.expires_at).toLocaleString() : 'the date set'}.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Keep the date</AlertDialog.Cancel>
			<AlertDialog.Action onclick={keepForever} data-testid="clear-expiry-confirm">
				Drop it
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
