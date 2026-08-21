<script lang="ts">
	// Discord alerts for one map: standing questions about the chain that answer themselves
	// into a channel.
	import BellIcon from '@lucide/svelte/icons/bell';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { page } from '$app/state';
	import { api, errorMessage } from '$lib/api/client';
	import type { AlertDelivery } from '$lib/api/types/AlertDelivery';
	import type { AlertKind } from '$lib/api/types/AlertKind';
	import type { AlertMention } from '$lib/api/types/AlertMention';
	import type { MapAlert } from '$lib/api/types/MapAlert';
	import type { MapAlertEvent } from '$lib/api/types/MapAlertEvent';
	import type { MapWebhook } from '$lib/api/types/MapWebhook';
	import type { MapWebhookRole } from '$lib/api/types/MapWebhookRole';
	import type { SaveAlert } from '$lib/api/types/SaveAlert';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Switch } from '$lib/components/ui/switch';
	import { timeAgo } from '$lib/format';
	import { cn } from '$lib/utils';
	import AlertForm from './AlertForm.svelte';
	import DestinationsCard from './DestinationsCard.svelte';

	const mapId = $derived(Number(page.params.id));

	let alerts = $state<MapAlert[]>([]);
	let events = $state<MapAlertEvent[]>([]);
	let webhooks = $state<MapWebhook[]>([]);
	let roles = $state<MapWebhookRole[]>([]);
	let error = $state<string | null>(null);
	let editing = $state<MapAlert | null>(null);
	let creating = $state(false);
	// The server is the authority on who may manage alerts, so nothing renders until the list
	// comes back: otherwise the controls would offer buttons that 403.
	let canManage = $state(false);

	async function load() {
		try {
			[alerts, events, webhooks, roles] = await Promise.all([
				api.listAlerts(mapId),
				api.alertEvents(mapId),
				api.listWebhooks(mapId),
				api.listAlertRoles(mapId),
			]);
			error = null;
			canManage = true;
		} catch (err) {
			error = errorMessage(err);
			canManage = false;
		}
	}

	$effect(() => {
		void mapId;
		load();
	});

	async function act(work: Promise<unknown>) {
		try {
			await work;
			error = null;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function save(body: SaveAlert) {
		const id = editing?.id;
		await act(id ? api.updateAlert(mapId, id, body) : api.createAlert(mapId, body));
		editing = null;
		creating = false;
	}

	function remove(alert: MapAlert) {
		if (!confirm(`Delete "${alert.name}"?`)) return;
		act(api.deleteAlert(mapId, alert.id));
	}

	const KIND_LABEL = {
		killmail: 'Kills near the chain',
		proximity: 'System near the chain',
		jump_range: 'Capital jump range',
	} satisfies Record<AlertKind, string>;
	const DELIVERY_LABEL = {
		webhook: 'Channel webhook',
		discord_dm: 'Direct message',
		discord_channel: 'Bot channel',
	} satisfies Record<AlertDelivery, string>;
	const MENTION_LABEL = {
		none: 'No ping',
		creator: 'Pings the creator',
		role: 'Pings a role',
		everyone: 'Pings everyone',
	} satisfies Record<AlertMention, string>;
	const REASON: Record<string, string> = {
		manual: 'Turned off by hand',
		discord_unlinked: 'The creator unlinked their Discord account',
		access_revoked: 'The creator lost access to this map',
		destination_gone: 'Discord rejected the destination: the webhook or channel is gone',
		delivery_failed: 'Too many failed deliveries',
	};

	const SHIP_LABEL: Record<string, string> = {
		dreadnought: 'Dreadnought',
		carrier: 'Carrier',
		force_auxiliary: 'Force Auxiliary',
		supercarrier: 'Supercarrier',
		titan: 'Titan',
		jump_freighter: 'Jump Freighter',
		rorqual: 'Rorqual',
		black_ops: 'Black Ops',
	};

	function summary(alert: MapAlert): string {
		const target = alert.target_system_name ?? 'a system';
		if (alert.kind === 'jump_range') {
			const ship = SHIP_LABEL[alert.ship_type ?? ''] ?? 'a capital';
			return `An exit within ${ship} range (JDC ${alert.jdc_level ?? 0}) of ${target}`;
		}
		const within = `within ${alert.max_jumps} ${alert.max_jumps === 1 ? 'jump' : 'jumps'}`;
		if (alert.kind === 'killmail') {
			const rules = alert.filters.length;
			return rules === 0
				? `Anything that dies ${within}`
				: `${rules} ${rules === 1 ? 'filter' : 'filters'} (${alert.filter_match}), ${within}`;
		}
		return `${target} ${within}`;
	}
</script>

<div class="flex flex-col gap-6">
	{#if error}
		<p class="text-sm text-destructive" data-testid="alerts-error">{error}</p>
	{/if}

	<Card.Root>
		<Card.Header>
			<div class="flex items-start justify-between gap-3">
				<div class="flex flex-col gap-1.5">
					<Card.Title class="flex items-center gap-2">
						<BellIcon class="size-4" />
						Discord alerts
					</Card.Title>
					<Card.Description>
						Standing questions about the chain, answered into a Discord channel.
					</Card.Description>
				</div>
				{#if canManage && !creating && !editing}
					<Button
						size="sm"
						onclick={() => {
							creating = true;
							editing = null;
						}}
						data-testid="alert-new"
					>
						<PlusIcon data-icon="inline-start" />
						New alert
					</Button>
				{/if}
			</div>
		</Card.Header>
		<Card.Content class="flex flex-col gap-3">
			{#if creating || editing}
				<!-- Keyed so switching alerts reseeds the form instead of keeping the old values. -->
				{#key editing?.id ?? 'new'}
					<AlertForm
						alert={editing}
						{webhooks}
						{roles}
						onsave={save}
						oncancel={() => {
							creating = false;
							editing = null;
						}}
					/>
				{/key}
			{/if}

			{#if canManage && alerts.length === 0 && !creating}
				<p class="py-6 text-center text-sm text-muted-foreground" data-testid="alerts-empty">
					No alerts yet. A channel webhook is the quickest way to start: Discord's channel settings,
					Integrations, New Webhook.
				</p>
			{/if}

			{#each alerts as alert (alert.id)}
				<div
					class={cn(
						'flex flex-col gap-2 border border-border/60 p-3',
						!alert.is_active && 'opacity-60',
					)}
					data-testid="alert-row"
					data-alert={alert.id}
				>
					<div class="flex items-start justify-between gap-3">
						<div class="flex min-w-0 flex-col gap-1">
							<span class="flex items-center gap-2">
								<span class="truncate text-sm font-medium">{alert.name}</span>
								<Badge variant="outline" class="shrink-0">{KIND_LABEL[alert.kind]}</Badge>
							</span>
							<span class="text-xs text-muted-foreground">{summary(alert)}</span>
							<span
								class="flex flex-wrap items-center gap-x-2 text-[11px] text-muted-foreground/70"
							>
								<span>{DELIVERY_LABEL[alert.delivery]}</span>
								{#if alert.webhook_name}
									<span>→ {alert.webhook_name}</span>
								{/if}
								{#if alert.mention === 'role' && alert.role_name}
									<span>@{alert.role_name}</span>
								{/if}
								<span>·</span>
								<span>{MENTION_LABEL[alert.mention]}</span>
								{#if alert.last_fired_at}
									<span>·</span>
									<span data-testid="alert-fired">fired {timeAgo(alert.last_fired_at)}</span>
								{/if}
							</span>
							{#if !alert.is_active && alert.disabled_reason}
								<span class="text-[11px] text-amber-500" data-testid="alert-reason">
									{REASON[alert.disabled_reason] ?? alert.disabled_reason}
								</span>
							{/if}
						</div>
						<div class="flex shrink-0 items-center gap-2">
							<Switch
								checked={alert.is_active}
								aria-label="Enable {alert.name}"
								onCheckedChange={(value) => act(api.setAlertActive(mapId, alert.id, value))}
							/>
							<Button
								variant="ghost"
								size="sm"
								onclick={() => {
									editing = alert;
									creating = false;
								}}
								data-testid="alert-edit">Edit</Button
							>
							<Button
								variant="ghost"
								size="icon"
								class="size-8"
								aria-label="Delete {alert.name}"
								onclick={() => remove(alert)}
							>
								<TrashIcon />
							</Button>
						</div>
					</div>
				</div>
			{/each}
		</Card.Content>
	</Card.Root>

	{#if canManage}
		<DestinationsCard {mapId} {webhooks} {roles} onchange={load} />
	{/if}

	<Card.Root>
		<Card.Header>
			<Card.Title>History</Card.Title>
			<Card.Description>Who changed what, and every time an alert fired or failed.</Card.Description
			>
		</Card.Header>
		<Card.Content>
			{#if events.length === 0}
				<p class="py-4 text-sm text-muted-foreground">Nothing yet.</p>
			{:else}
				<ul class="flex flex-col text-xs" data-testid="alert-events">
					{#each events as event (event.id)}
						<li class="flex items-center gap-2 border-b border-border/30 py-1.5 last:border-b-0">
							<span class="w-16 shrink-0 font-mono text-[10px] text-muted-foreground uppercase">
								{event.kind}
							</span>
							<span class="min-w-0 flex-1 truncate">
								{event.alert_name ?? event.detail ?? '—'}
								{#if event.detail && event.alert_name}
									<span class="text-muted-foreground">: {event.detail}</span>
								{/if}
							</span>
							<span class="shrink-0 text-muted-foreground">{event.actor ?? 'system'}</span>
							<span class="w-16 shrink-0 text-right text-muted-foreground/70">
								{timeAgo(event.created_at)}
							</span>
						</li>
					{/each}
				</ul>
			{/if}
		</Card.Content>
	</Card.Root>
</div>
