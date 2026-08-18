<script lang="ts">
	// Where alerts can post, and who they can ping. Registered once per map and pointed at
	// by name, so a webhook URL is pasted once rather than into every alert, and rotating
	// one is a single edit. Roles get names for the same reason: nobody recognises
	// 1189734502938472 as the scouts.
	import PlusIcon from '@lucide/svelte/icons/plus';
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { api } from '$lib/api/client';
	import type { MapWebhook } from '$lib/api/types/MapWebhook';
	import type { MapWebhookRole } from '$lib/api/types/MapWebhookRole';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';

	let {
		mapId,
		webhooks,
		roles,
		onchange
	}: {
		mapId: number;
		webhooks: MapWebhook[];
		roles: MapWebhookRole[];
		onchange: () => void;
	} = $props();

	let webhookName = $state('');
	let webhookUrl = $state('');
	let roleName = $state('');
	let roleId = $state('');
	let error = $state<string | null>(null);

	async function act(work: Promise<unknown>) {
		try {
			await work;
			error = null;
			onchange();
		} catch (err) {
			error = (err as Error).message;
		}
	}

	async function addWebhook() {
		await act(api.createWebhook(mapId, { name: webhookName.trim(), url: webhookUrl.trim() }));
		webhookName = '';
		webhookUrl = '';
	}

	async function addRole() {
		await act(
			api.createAlertRole(mapId, { name: roleName.trim(), discord_role_id: roleId.trim() })
		);
		roleName = '';
		roleId = '';
	}

	function removeWebhook(webhook: MapWebhook) {
		const warning =
			webhook.alert_count > 0
				? `Delete "${webhook.name}"? ${webhook.alert_count} alert${webhook.alert_count === 1 ? '' : 's'} posting there will be deleted too.`
				: `Delete "${webhook.name}"?`;
		if (!confirm(warning)) return;
		act(api.deleteWebhook(mapId, webhook.id));
	}
</script>

<Card.Root>
	<Card.Header>
		<Card.Title>Destinations and roles</Card.Title>
		<Card.Description>
			Register a channel once, then point any number of alerts at it.
		</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-col gap-6">
		{#if error}
			<p class="text-sm text-destructive" data-testid="destinations-error">{error}</p>
		{/if}

		<div class="flex flex-col gap-2">
			<span class="text-sm font-medium">Destinations</span>
			{#each webhooks as webhook (webhook.id)}
				<div
					class="flex items-center justify-between gap-3 border border-border/60 px-3 py-2"
					data-testid="destination-row"
				>
					<span class="flex min-w-0 flex-col">
						<span class="truncate text-sm">{webhook.name}</span>
						<span class="truncate font-mono text-[11px] text-muted-foreground">
							{webhook.summary}
						</span>
					</span>
					<span class="flex shrink-0 items-center gap-2">
						{#if webhook.alert_count > 0}
							<span class="text-xs text-muted-foreground">
								{webhook.alert_count}
								{webhook.alert_count === 1 ? 'alert' : 'alerts'}
							</span>
						{/if}
						<Button
							variant="ghost"
							size="icon"
							class="size-8"
							aria-label="Delete {webhook.name}"
							onclick={() => removeWebhook(webhook)}
						>
							<TrashIcon />
						</Button>
					</span>
				</div>
			{/each}
			<div class="flex flex-wrap gap-2">
				<Input
					class="w-40"
					placeholder="Name"
					bind:value={webhookName}
					data-testid="destination-name"
				/>
				<Input
					class="min-w-56 flex-1"
					type="password"
					placeholder="https://discord.com/api/webhooks/…"
					bind:value={webhookUrl}
					data-testid="destination-url"
				/>
				<Button
					variant="outline"
					disabled={!webhookName.trim() || !webhookUrl.trim()}
					onclick={addWebhook}
					data-testid="destination-add"
				>
					<PlusIcon data-icon="inline-start" />
					Add
				</Button>
			</div>
			<p class="text-xs text-muted-foreground">
				Discord: channel settings → Integrations → New Webhook. The URL is stored write-only
				and never shown again.
			</p>
		</div>

		<div class="flex flex-col gap-2">
			<span class="text-sm font-medium">Roles to ping</span>
			{#each roles as role (role.id)}
				<div
					class="flex items-center justify-between gap-3 border border-border/60 px-3 py-2"
					data-testid="role-row"
				>
					<span class="flex min-w-0 flex-col">
						<span class="truncate text-sm">{role.name}</span>
						<span class="truncate font-mono text-[11px] text-muted-foreground">
							{role.discord_role_id}
						</span>
					</span>
					<Button
						variant="ghost"
						size="icon"
						class="size-8 shrink-0"
						aria-label="Delete {role.name}"
						onclick={() => act(api.deleteAlertRole(mapId, role.id))}
					>
						<TrashIcon />
					</Button>
				</div>
			{/each}
			<div class="flex flex-wrap gap-2">
				<Input class="w-40" placeholder="Name" bind:value={roleName} data-testid="role-name" />
				<Input
					class="min-w-56 flex-1"
					placeholder="Discord role id"
					bind:value={roleId}
					data-testid="role-id"
				/>
				<Button
					variant="outline"
					disabled={!roleName.trim() || !roleId.trim()}
					onclick={addRole}
					data-testid="role-add"
				>
					<PlusIcon data-icon="inline-start" />
					Add
				</Button>
			</div>
			<p class="text-xs text-muted-foreground">
				Turn on Developer Mode in Discord, then right-click a role and Copy ID.
			</p>
		</div>
	</Card.Content>
</Card.Root>
