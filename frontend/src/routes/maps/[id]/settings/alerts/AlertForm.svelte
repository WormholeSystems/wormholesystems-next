<script lang="ts">
	// Creating or editing one alert. The form changes shape with the kind: a killmail alert
	// asks whose kills, a proximity alert asks which system.
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';

	import type { AlertDelivery } from '$lib/api/types/AlertDelivery';
	import type { AlertKind } from '$lib/api/types/AlertKind';
	import type { AlertMention } from '$lib/api/types/AlertMention';
	import type { JumpShip } from '$lib/api/types/JumpShip';
	import type { MapAlert } from '$lib/api/types/MapAlert';
	import type { MapWebhook } from '$lib/api/types/MapWebhook';
	import type { MapWebhookRole } from '$lib/api/types/MapWebhookRole';
	import type { Rule } from '$lib/api/types/Rule';
	import type { SaveAlert } from '$lib/api/types/SaveAlert';
	import type { Side } from '$lib/api/types/Side';
	import type { Subject } from '$lib/api/types/Subject';
	import { resolveCache } from '$lib/resolve-cache.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import SystemCombobox from '$lib/components/pickers/SystemCombobox.svelte';

	let {
		alert,
		webhooks,
		roles,
		onsave,
		oncancel,
	}: {
		alert: MapAlert | null;
		webhooks: MapWebhook[];
		roles: MapWebhookRole[];
		onsave: (body: SaveAlert) => void;
		oncancel: () => void;
	} = $props();

	// Seeded once on purpose: this is an editing buffer, and the parent remounts the form
	// (keyed on the alert) when you switch to a different one.
	/* svelte-ignore state_referenced_locally */
	const seed = alert;
	let name = $state(seed?.name ?? '');
	let kind = $state<AlertKind>(seed?.kind ?? 'killmail');
	let delivery = $state<AlertDelivery>(seed?.delivery ?? 'webhook');
	let webhookId = $state<number | null>(seed?.map_webhook_id ?? null);
	let mention = $state<AlertMention>(seed?.mention ?? 'none');
	let roleRef = $state<number | null>(seed?.map_webhook_role_id ?? null);
	let channelId = $state(seed?.discord_channel_id ?? '');
	let target = $state<number | null>(seed?.target_solar_system_id ?? null);
	let maxJumps = $state(seed?.max_jumps ?? 5);
	let shipType = $state<JumpShip>(seed?.ship_type ?? 'dreadnought');
	let jdcLevel = $state(seed?.jdc_level ?? 5);
	let filters = $state<Rule[]>(seed ? structuredClone($state.snapshot(seed.filters)) : []);
	let filterMatch = $state(seed?.filter_match ?? 'any');

	const KINDS: { value: AlertKind; label: string; blurb: string }[] = [
		{
			value: 'killmail',
			label: 'Kills near the chain',
			blurb: 'Every kill within reach, optionally narrowed to who is involved.',
		},
		{
			value: 'proximity',
			label: 'System near the chain',
			blurb: 'Fires when the chain comes within gate range of a system you name.',
		},
		{
			value: 'jump_range',
			label: 'Capital jump range',
			blurb:
				'Fires when a k-space exit lands within a capital jump of a system you name. ' +
				'Measured in light years for the hull you pick, not in gates.',
		},
	];
	const SHIPS: { value: JumpShip; label: string; base: number }[] = [
		{ value: 'dreadnought', label: 'Dreadnought', base: 3.5 },
		{ value: 'carrier', label: 'Carrier', base: 3.5 },
		{ value: 'force_auxiliary', label: 'Force Auxiliary', base: 3.5 },
		{ value: 'supercarrier', label: 'Supercarrier', base: 3.0 },
		{ value: 'titan', label: 'Titan', base: 3.0 },
		{ value: 'jump_freighter', label: 'Jump Freighter', base: 5.0 },
		{ value: 'rorqual', label: 'Rorqual', base: 5.0 },
		{ value: 'black_ops', label: 'Black Ops', base: 4.0 },
	];
	// The same arithmetic the server does: "JDC 5" means nothing until it is light years.
	const range = $derived(
		((SHIPS.find((s) => s.value === shipType)?.base ?? 3.5) * (1 + 0.2 * jdcLevel)).toFixed(1),
	);
	const DELIVERIES: { value: AlertDelivery; label: string }[] = [
		{ value: 'webhook', label: 'Channel webhook' },
	];
	const MENTIONS: { value: AlertMention; label: string }[] = [
		{ value: 'none', label: 'No ping' },
		{ value: 'role', label: 'Ping a role' },
		{ value: 'everyone', label: 'Ping everyone' },
	];
	const SUBJECTS: { value: Subject; label: string }[] = [
		{ value: 'alliance', label: 'Alliance' },
		{ value: 'corporation', label: 'Corporation' },
		{ value: 'character', label: 'Character' },
		{ value: 'ship_type', label: 'Ship type' },
		{ value: 'ship_group', label: 'Ship group' },
	];
	const SIDES: { value: Side; label: string }[] = [
		{ value: 'either', label: 'either side' },
		{ value: 'victim', label: 'the victim' },
		{ value: 'attacker', label: 'the killer' },
	];

	// The picker wants a resolved system for its label; the alert only stores the id.
	const systems = resolveCache();
	$effect(() => {
		if (target !== null) systems.ensure([target]);
	});
	const targetSystem = $derived(target === null ? null : (systems.get(target) ?? null));

	function addRule() {
		filters = [...filters, { subject: 'alliance', side: 'either', mode: 'include', ids: [] }];
	}

	function removeRule(index: number) {
		filters = filters.filter((_, i) => i !== index);
	}

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
			(kind === 'killmail' || target !== null) &&
			(delivery !== 'webhook' || webhookId !== null) &&
			(mention !== 'role' || roleRef !== null),
	);

	function submit() {
		onsave({
			name: name.trim(),
			kind,
			delivery,
			map_webhook_id: delivery === 'webhook' ? (webhookId ?? undefined) : undefined,
			discord_channel_id: channelId.trim() || undefined,
			map_webhook_role_id: mention === 'role' ? (roleRef ?? undefined) : undefined,
			mention,
			target_solar_system_id: kind === 'killmail' ? undefined : (target ?? undefined),
			max_jumps: maxJumps,
			ship_type: kind === 'jump_range' ? shipType : undefined,
			jdc_level: kind === 'jump_range' ? jdcLevel : undefined,
			filters: kind === 'killmail' ? filters.filter((r) => r.ids.length > 0) : [],
			filter_match: filterMatch,
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
			<SystemCombobox placeholder="Pick a system" value={target} onpick={(id) => (target = id)} />
			{#if targetSystem}
				<p class="text-xs text-muted-foreground">
					{targetSystem.name} · {targetSystem.region}
				</p>
			{/if}
		</div>
	{/if}

	{#if kind === 'jump_range'}
		<div class="flex flex-col gap-1.5">
			<span class="text-sm font-medium">In range of</span>
			<div class="flex flex-wrap items-center gap-2">
				<Select.Root type="single" bind:value={shipType}>
					<Select.Trigger class="w-48" data-testid="alert-ship">
						{SHIPS.find((s) => s.value === shipType)?.label}
					</Select.Trigger>
					<Select.Content>
						<Select.Group>
							{#each SHIPS as option (option.value)}
								<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
							{/each}
						</Select.Group>
					</Select.Content>
				</Select.Root>
				<span class="text-sm text-muted-foreground">at JDC</span>
				<Input
					type="number"
					min="0"
					max="5"
					class="w-20"
					value={jdcLevel}
					oninput={(e) => (jdcLevel = Number((e.currentTarget as HTMLInputElement).value))}
					data-testid="alert-jdc"
				/>
				<span class="text-sm text-muted-foreground" data-testid="alert-range">
					= {range} ly
				</span>
			</div>
		</div>
	{:else}
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
	{/if}

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
								i === index ? { ...r, mode: value as Rule['mode'] } : r,
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
								i === index ? { ...r, subject: value as Subject } : r,
							))}
					>
						<Select.Trigger class="h-8 w-36">
							{SUBJECTS.find((s) => s.value === rule.subject)?.label}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								{#each SUBJECTS as option (option.value)}
									<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item
									>
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
									<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item
									>
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
			{#if webhooks.length === 0}
				<p class="text-xs text-amber-500" data-testid="alert-no-destinations">
					No destinations registered yet. Add one below first.
				</p>
			{:else}
				<Select.Root
					type="single"
					value={webhookId === null ? '' : String(webhookId)}
					onValueChange={(value) => (webhookId = value ? Number(value) : null)}
				>
					<Select.Trigger class="w-full" data-testid="alert-destination">
						{webhooks.find((w) => w.id === webhookId)?.name ?? 'Pick a destination'}
					</Select.Trigger>
					<Select.Content>
						<Select.Group>
							{#each webhooks as webhook (webhook.id)}
								<Select.Item value={String(webhook.id)} label={webhook.name}>
									{webhook.name}
								</Select.Item>
							{/each}
						</Select.Group>
					</Select.Content>
				</Select.Root>
			{/if}
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
				{#if roles.length === 0}
					<p class="self-center text-xs text-amber-500">No roles registered yet.</p>
				{:else}
					<Select.Root
						type="single"
						value={roleRef === null ? '' : String(roleRef)}
						onValueChange={(value) => (roleRef = value ? Number(value) : null)}
					>
						<Select.Trigger class="flex-1" data-testid="alert-role">
							{roles.find((r) => r.id === roleRef)?.name ?? 'Pick a role'}
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								{#each roles as role (role.id)}
									<Select.Item value={String(role.id)} label={role.name}>{role.name}</Select.Item>
								{/each}
							</Select.Group>
						</Select.Content>
					</Select.Root>
				{/if}
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
