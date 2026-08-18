<script lang="ts">
	// Linking this Vector account to a Discord one.
	//
	// Per account, not per map: the link answers "which Discord user is this", which is what
	// a direct message and a slash command both need, whichever map they are about.
	import CheckCircleIcon from '@lucide/svelte/icons/check-circle-2';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';

	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { DiscordAccount } from '$lib/api/types/DiscordAccount';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';

	let account = $state<DiscordAccount | null>(null);
	let loaded = $state(false);
	let error = $state<string | null>(null);

	const justLinked = $derived(page.url.searchParams.get('linked') === '1');

	async function load() {
		try {
			account = await api.myDiscord();
			error = null;
		} catch (err) {
			error = (err as Error).message;
		} finally {
			loaded = true;
		}
	}

	$effect(() => {
		load();
	});

	async function unlink() {
		if (!confirm('Unlink Discord? Alerts that direct-message you will stop.')) return;
		try {
			await api.unlinkDiscord();
			await load();
		} catch (err) {
			error = (err as Error).message;
		}
	}
</script>

<div class="mx-auto flex max-w-2xl flex-col gap-6 py-6">
	<h1 class="font-heading text-lg font-semibold tracking-tight">Discord</h1>

	{#if error}
		<p class="text-sm text-destructive" data-testid="discord-error">{error}</p>
	{/if}

	<Card.Root>
		<Card.Header>
			<Card.Title>Your Discord account</Card.Title>
			<Card.Description>
				Linking lets the bot know which maps are yours, so <code>/vector</code> can answer about
				them and alerts can reach you directly.
			</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col gap-4">
			{#if !loaded}
				<p class="text-sm text-muted-foreground">Checking…</p>
			{:else if account}
				<div class="flex items-center justify-between gap-3" data-testid="discord-linked">
					<span class="flex items-center gap-2 text-sm">
						<CheckCircleIcon class="size-4 text-emerald-500" />
						Linked to
						<span class="font-medium">{account.display_name ?? account.username}</span>
						<span class="font-mono text-xs text-muted-foreground">@{account.username}</span>
					</span>
					<Button variant="outline" size="sm" onclick={unlink} data-testid="discord-unlink">
						Unlink
					</Button>
				</div>
				{#if justLinked}
					<p class="text-xs text-emerald-500">Linked. Try <code>/vector account</code> in Discord.</p>
				{/if}
			{:else}
				<p class="text-sm text-muted-foreground" data-testid="discord-unlinked">
					No Discord account linked yet.
				</p>
				<Button href="/discord/connect" class="w-fit" data-testid="discord-connect">
					<ExternalLinkIcon data-icon="inline-start" />
					Connect Discord
				</Button>
			{/if}
		</Card.Content>
	</Card.Root>

	<Card.Root>
		<Card.Header>
			<Card.Title>Commands</Card.Title>
			<Card.Description>What the bot answers, wherever you ask it.</Card.Description>
		</Card.Header>
		<Card.Content>
			<ul class="flex flex-col gap-2 text-sm">
				<li>
					<code class="text-foreground">/vector account</code>
					<span class="text-muted-foreground"> — which Vector account this Discord user is.</span>
				</li>
				<li>
					<code class="text-foreground">/vector alerts</code>
					<span class="text-muted-foreground"> — the alerts you created, and whether they are on.</span>
				</li>
				<li>
					<code class="text-foreground">/vector route</code>
					<span class="text-muted-foreground">
						— how far a system is from one of your chains, counting wormholes as free.
					</span>
				</li>
			</ul>
			<p class="mt-3 text-xs text-muted-foreground">
				Replies are only visible to you, even in a busy channel.
			</p>
		</Card.Content>
	</Card.Root>
</div>
