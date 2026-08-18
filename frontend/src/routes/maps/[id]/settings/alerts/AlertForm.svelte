<script lang="ts">
	// Creating or editing one alert.
	//
	// The form changes shape with the kind, because the kinds ask different questions: a
	// killmail alert wants to know whose kills, a proximity alert wants to know which system.
	// Showing both at once and greying half out would be more chrome for less clarity.
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';

	import { api } from '$lib/api/client';
	import type { AlertDelivery } from '$lib/api/types/AlertDelivery';
	import type { AlertKind } from '$lib/api/types/AlertKind';
	import type { AlertMention } from '$lib/api/types/AlertMention';
	import type { MapAlert } from '$lib/api/types/MapAlert';
	import type { Rule } from '$lib/api/types/Rule';
	import type { SaveAlert } from '$lib/api/types/SaveAlert';
	import type { Side } from '$lib/api/types/Side';
	import type { Subject } from '$lib/api/types/Subject';
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import SystemCombobox from '$lib/components/pickers/SystemCombobox.svelte';

	let {
		alert,
		onsave,
		oncancel
	}: {
		alert: MapAlert | null;
		onsave: (body: SaveAlert) => void;
		oncancel: () => void;
	} = $props();

	// Seeded from the alert once, on purpose: this is an editing buffer, and the parent
	// remounts the form (keyed on the alert) when you switch to a different one.
	/* svelte-ignore state_referenced_locally */
	const seed = alert;
	let name = $state(seed?.name ?? '');
	let kind = $state<AlertKind>(seed?.kind ?? 'killmail');
	let delivery = $state<AlertDelivery>(seed?.delivery ?? 'webhook');
	let webhookUrl = $state('');
	let mention = $state<AlertMention>(seed?.mention ?? 'none');
	let roleId = $state(seed?.discord_role_id ?? '');
	let channelId = $state(seed?.discord_channel_id ?? '');
	let target = $state<number | null>(seed?.target_solar_system_id ?? null);
	let maxJumps = $state(seed?.max_jumps ?? 5);
	let filters = $state<Rule[]>(seed ? structuredClone($state.snapshot(seed.filters)) : []);
	let filterMatch = $state(seed?.filter_match ?? 'any');

	const KINDS: { value: AlertKind; label: string; blurb: string }[] = [
		{
			value: 'killmail',
			label: 'Kills near the chain',
			blurb: 'Every kill within reach, optionally narrowed to who is involved.'
		},
		{
			value: 'proximity',
			label: 'System near the chain',
			blurb: 'Fires when the chain comes within range of a system you name.'
		}
	];
	const DELIVERIES: { value: AlertDelivery; label: string }[] = [
		{ value: 'webhook', label: 'Channel webhook' }
	];
	const MENTIONS: { value: AlertMention; label: string }[] = [
		{ value: 'none', label: 'No ping' },
		{ value: 'role', label: 'Ping a role' },
		{ value: 'everyone', label: 'Ping everyone' }
	];
	const SUBJECTS: { value: Subject; label: string }[] = [
		{ value: 'alliance', label: 'Alliance' },
		{ value: 'corporation', label: 'Corporation' },
		{ value: 'character', label: 'Character' },
		{ value: 'ship_type', label: 'Ship type' },
		{ value: 'ship_group', label: 'Ship group' }
	];
	const SIDES: { value: Side; label: string }[] = [
		{ value: 'either', label: 'either side' },
		{ value: 'victim', label: 'the victim' },
		{ value: 'attacker', label: 'the killer' }
	];

	// The picker wants a resolved system for its label; the alert only stores the id.
	let targetSystem = $state<SystemSearchResult | null>(null);
	$effect(() => {
		const id = target;
		if (id === null) {
			targetSystem = null;
			return;
		}
		api
			.resolveSystems([id])
			.then(([hit]) => (targetSystem = hit ?? null))
			.catch(() => {});
	});

	function addRule() {
		filters = [...filters, { subject: 'alliance', side: 'either', mode: 'include', ids: [] }];
	}

	function removeRule(index: number) {
		filters = filters.filter((_, i) => i !== index);
	}

	/** Ids are typed as a list because that is what they are: "any of these alliances". */
	function idsOf(rule: Rule): string {
		return rule.ids.join(', ');
	}

	function setIds(index: number, value: string) {
		const ids = value
			.split(',')
			.map((part) => Number(part.trim()))
			.filter((id) => Number.isFinite(id) && id > 0);
		filters = filters.map((rule, i) => (i === index ? { ...rule, ids } : rule));
	}

	const valid = $derived(
		name.trim().length > 0 &&
			(kind !== 'proximity' || target !== null) &&
			(delivery !== 'webhook' || alert !== null || webhookUrl.trim().length > 0) &&
			(mention !== 'role' || roleId.trim().length > 0)
	);

	function submit() {
		onsave({
			name: name.trim(),
			kind,
			delivery,
			// Left out on an edit unless retyped, so the stored secret survives a rename.
			webhook_url: webhookUrl.trim() || undefined,
			discord_channel_id: channelId.trim() || undefined,
			discord_role_id: roleId.trim() || undefined,
			mention,
			target_solar_system_id: kind === 'killmail' ? undefined : (target ?? undefined),
			max_jumps: maxJumps,
			filters: kind === 'killmail' ? filters.filter((r) => r.ids.length > 0) : [],
			filter_match: filterMatch
		});
	}
</script>

<div class="flex flex-col gap-4 border border-border bg-muted/20 p-4" data-testid="alert-form">
	<div class="flex flex-col gap-1.5">
		<label for="alert-name" class="text-sm font-medium">Name</label>
		<Input
			id="alert-name"
			bind:value={name}
			placeholder="Kills around home"
			data-testid="alert-name"
		/>
	</div>

	<div class="flex flex-col gap-1.5">
		<span class="text-sm font-medium">What to watch for</span>
		<Select.Root type="single" bind:value={kind}>
			<Select.Trigger class="w-full" data-testid="alert-kind">
				{KINDS.find((k) => k.value === kind)?.label}
			</Select.Trigger>
			<Select.Content>
				<Select.Group>
					{#each KINDS as option (option.value)}
						<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
					{/each}
				</Select.Group>
			</Select.Content>
		</Select.Root>
		<p class="text-xs text-muted-foreground">{KINDS.find((k) => k.value === kind)?.blurb}</p>
	</div>

	{#if kind !== 'killmail'}
		<div class="flex flex-col gap-1.5">
			<span class="text-sm font-medium">System to watch</span>
			<SystemCombobox
				placeholder="Pick a system"
				value={target}
				onpick={(id) => (target = id)}
			/>
			{#if targetSystem}
				<p class="text-xs text-muted-foreground">
					{targetSystem.name} · {targetSystem.region}
				</p>
			{/if}
		</div>
	{/if}

	<div class="flex flex-col gap-1.5">
		<label for="alert-jumps" class="text-sm font-medium">Within</label>
		<div class="flex items-center gap-2">
			<Input
				id="alert-jumps"
				type="number"
				min="0"
				max="30"
				class="w-24"
				value={maxJumps}
				oninput={(e) => (maxJumps = Number((e.currentTarget as HTMLInputElement).value))}
				data-testid="alert-jumps"
			/>
			<span class="text-sm text-muted-foreground">
				gate jumps of the chain, counting wormholes as free
			</span>
		</div>
	</div>

	{#if kind === 'killmail'}
		<div class="flex flex-col gap-2">
			<div class="flex items-center justify-between">
				<span class="text-sm font-medium">Filters</span>
				<Button variant="outline" size="sm" onclick={addRule} data-testid="alert-add-filter">
					<PlusIcon data-icon="inline-start" />
					Add rule
				</Button>
			</div>
			{#if filters.length === 0}
				<p class="text-xs text-muted-foreground">
					No rules: every kill within range is worth a message.
				</p>
			{:else}
				<div class="flex items-center gap-2 text-xs text-muted-foreground">
					<span>Fire when</span>
					<Select.Root type="single" bind:value={filterMatch}>
						<Select.Trigger class="h-7 w-28" data-testid="alert-filter-match">
							{filterMatch === 'all' ? 'all rules' : 'any rule'}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								<Select.Item value="any" label="any rule">any rule</Select.Item>
								<Select.Item value="all" label="all rules">all rules</Select.Item>
							</Select.Group>
						</Select.Content>
					</Select.Root>
					<span>matches. An exclusion always wins.</span>
				</div>
			{/if}

			{#each filters as rule, index (index)}
				<div class="flex flex-wrap items-center gap-2" data-testid="alert-filter">
					<Select.Root
						type="single"
						value={rule.mode}
						onValueChange={(value) =>
							(filters = filters.map((r, i) =>
								i === index ? { ...r, mode: value as Rule['mode'] } : r
							))}
					>
						<Select.Trigger class="h-8 w-28">{rule.mode}</Select.Trigger>
						<Select.Content>
							<Select.Group>
								<Select.Item value="include" label="include">include</Select.Item>
								<Select.Item value="exclude" label="exclude">exclude</Select.Item>
							</Select.Group>
						</Select.Content>
					</Select.Root>
					<Select.Root
						type="single"
						value={rule.subject}
						onValueChange={(value) =>
							(filters = filters.map((r, i) =>
								i === index ? { ...r, subject: value as Subject } : r
							))}
					>
						<Select.Trigger class="h-8 w-36">
							{SUBJECTS.find((s) => s.value === rule.subject)?.label}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								{#each SUBJECTS as option (option.value)}
									<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
								{/each}
							</Select.Group>
						</Select.Content>
					</Select.Root>
					<Select.Root
						type="single"
						value={rule.side}
						onValueChange={(value) =>
							(filters = filters.map((r, i) => (i === index ? { ...r, side: value as Side } : r)))}
					>
						<Select.Trigger class="h-8 w-32">
							{SIDES.find((s) => s.value === rule.side)?.label}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								{#each SIDES as option (option.value)}
									<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
								{/each}
							</Select.Group>
						</Select.Content>
					</Select.Root>
					<Input
						class="h-8 min-w-40 flex-1"
						placeholder="EVE ids, comma separated"
						value={idsOf(rule)}
						oninput={(e) => setIds(index, (e.currentTarget as HTMLInputElement).value)}
						data-testid="alert-filter-ids"
					/>
					<Button
						variant="ghost"
						size="icon"
						class="size-8"
						aria-label="Remove rule"
						onclick={() => removeRule(index)}
					>
						<XIcon />
					</Button>
				</div>
			{/each}
		</div>
	{/if}

	<div class="flex flex-col gap-1.5">
		<span class="text-sm font-medium">Where it goes</span>
		<Select.Root type="single" bind:value={delivery}>
			<Select.Trigger class="w-full" data-testid="alert-delivery">
				{DELIVERIES.find((d) => d.value === delivery)?.label}
			</Select.Trigger>
			<Select.Content>
				<Select.Group>
					{#each DELIVERIES as option (option.value)}
						<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
					{/each}
				</Select.Group>
			</Select.Content>
		</Select.Root>
		{#if delivery === 'webhook'}
			<Input
				bind:value={webhookUrl}
				type="password"
				placeholder={alert
					? 'Leave blank to keep the current webhook'
					: 'https://discord.com/api/webhooks/…'}
				data-testid="alert-webhook"
			/>
			<p class="text-xs text-muted-foreground">
				Discord: channel settings → Integrations → New Webhook. The URL is a key to that
				channel, so it is stored write-only and never shown again.
			</p>
		{/if}
	</div>

	<div class="flex flex-col gap-1.5">
		<span class="text-sm font-medium">Ping</span>
		<div class="flex gap-2">
			<Select.Root type="single" bind:value={mention}>
				<Select.Trigger class="w-48" data-testid="alert-mention">
					{MENTIONS.find((m) => m.value === mention)?.label}
				</Select.Trigger>
				<Select.Content>
					<Select.Group>
						{#each MENTIONS as option (option.value)}
							<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
						{/each}
					</Select.Group>
				</Select.Content>
			</Select.Root>
			{#if mention === 'role'}
				<Input
					bind:value={roleId}
					class="flex-1"
					placeholder="Discord role id"
					data-testid="alert-role"
				/>
			{/if}
		</div>
	</div>

	<div class="flex justify-end gap-2">
		<Button variant="outline" size="sm" onclick={oncancel}>Cancel</Button>
		<Button size="sm" disabled={!valid} onclick={submit} data-testid="alert-save">
			{alert ? 'Save' : 'Create alert'}
		</Button>
	</div>
</div>
