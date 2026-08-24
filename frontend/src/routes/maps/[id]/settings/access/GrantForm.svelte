<script lang="ts">
	// The grant line: pick a subject (or paste an EVE id), a role, and optionally an end
	// date. Emits the finished grant and clears itself once the caller is done with it.
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import { getLocalTimeZone, today, type DateValue } from '@internationalized/date';

	import { q } from '$lib/api/queries';
	import type { AccessSubject } from '$lib/api/types/AccessSubject';
	import type { Role } from '$lib/api/types/Role';
	import EveImage from '$lib/components/EveImage.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Calendar } from '$lib/components/ui/calendar';
	import * as Command from '$lib/components/ui/command';
	import * as Popover from '$lib/components/ui/popover';
	import * as Select from '$lib/components/ui/select';
	import { searchQuery } from '$lib/search-query.svelte';
	import { ROLE_HELP, ROLE_LABEL } from '$lib/map/roles';

	let {
		ongrant,
	}: {
		/** Resolves once the grant is through (or refused); the form then clears itself. */
		ongrant: (grant: {
			subject_type: AccessSubject['subject_type'];
			subject_id: number;
			role: Role;
			expires_at: string | null;
		}) => Promise<unknown>;
	} = $props();

	let query = $state('');
	let picked = $state<AccessSubject | null>(null);
	let picking = $state(false);
	let newRole = $state<Role>('member');

	// Ownership is not grantable here: it is handed on from the General section instead.
	const ROLES: Role[] = ['viewer', 'member', 'manager'];
	const ALL_ROLES: Role[] = ['viewer', 'member', 'manager', 'owner'];

	const search = searchQuery({
		term: () => query,
		query: (settled) => q.searchAccessSubjects(settled),
	});
	const matches = $derived(search.results);

	function choose(subject: AccessSubject) {
		picked = subject;
		picking = false;
	}

	// Access for one operation is the case people forget to tidy up, so the form can set an
	// end from the start.
	const DURATIONS = [
		{ hours: 12, label: 'For 12 hours' },
		{ hours: 24, label: 'For a day' },
		{ hours: 24 * 7, label: 'For a week' },
		{ hours: 24 * 30, label: 'For a month' },
	];
	/** `null` is the permanent grant; a date ends at the close of that day. */
	let ends = $state<Date | null>(null);
	let picking_date = $state(false);
	let customDate = $state<DateValue | undefined>(undefined);

	const endsLabel = $derived(
		ends === null
			? 'No end date'
			: ends.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }),
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
		ongrant({
			subject_type: subject.subject_type,
			subject_id: subject.subject_id,
			role: newRole,
			expires_at: ends?.toISOString() ?? null,
		})
			.then(() => {
				query = '';
				picked = null;
				ends = null;
				customDate = undefined;
			})
			.catch(() => {});
	}
</script>

<div class="flex flex-col gap-2">
	<label for="grant-search" class="text-sm font-medium"> Add a character, corp or alliance </label>
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
		<Button onclick={grant} disabled={!picked && !Number(query.trim())} data-testid="grant-button">
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
