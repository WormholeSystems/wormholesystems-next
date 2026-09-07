<script lang="ts">
	// Personal access tokens: the keys a script presents instead of signing in. The secret
	// is shown once, straight after creation; the list afterwards never carries it.
	import CopyIcon from '@lucide/svelte/icons/copy';
	import TrashIcon from '@lucide/svelte/icons/trash-2';

	import { createQuery, useQueryClient } from '@tanstack/svelte-query';
	import { toast } from 'svelte-sonner';

	import { page } from '$app/state';
	import { api, errorMessage } from '$lib/api/client';
	import { apiAction } from '$lib/api/mutations';
	import { key, q } from '$lib/api/queries';
	import type { PersonalAccessToken } from '$lib/api/types/PersonalAccessToken';
	import { copyText } from '$lib/clipboard';
	import { confirmDanger } from '$lib/confirm.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { timeAgo, utcShort } from '$lib/format';

	const tokensQuery = createQuery(() => q.myTokens());
	const tokens = $derived(tokensQuery.data ?? []);
	const error = $derived(tokensQuery.error ? errorMessage(tokensQuery.error) : null);
	const client = useQueryClient();

	const lifetimes = [
		{ value: 'never', label: 'Until revoked' },
		{ value: '30', label: '30 days' },
		{ value: '90', label: '90 days' },
		{ value: '365', label: 'A year' },
	];

	let name = $state('');
	let lifetime = $state('never');
	let creating = $state(false);
	let created = $state<{ name: string; plaintext: string } | null>(null);

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (!name.trim() || creating) return;
		creating = true;
		try {
			const result = await api.createToken({
				name: name.trim(),
				expires_in_days: lifetime === 'never' ? undefined : Number(lifetime),
			});
			created = { name: result.token.name, plaintext: result.plaintext };
			name = '';
			await client.invalidateQueries({ queryKey: key.myTokens });
		} catch (err) {
			toast.error(errorMessage(err));
		} finally {
			creating = false;
		}
	}

	const act = apiAction(() => [key.myTokens]);

	async function revoke(token: PersonalAccessToken) {
		const sure = await confirmDanger({
			title: `Revoke ${token.name}?`,
			body: 'Whatever uses it is locked out on its next request. This cannot be undone.',
			action: 'Revoke',
		});
		if (!sure) return;
		if (created?.name === token.name) created = null;
		act.mutate(() => api.revokeToken(token.id));
	}

	function expiry(token: PersonalAccessToken): string {
		if (!token.expires_at) return 'Does not expire';
		const expired = new Date(token.expires_at).getTime() < Date.now();
		return `${expired ? 'Expired' : 'Expires'} ${utcShort(token.expires_at)}`;
	}
</script>

<div class="flex flex-col gap-6">
	{#if error}
		<p class="text-sm text-destructive" data-testid="tokens-error">{error}</p>
	{/if}

	<Card.Root>
		<Card.Header>
			<Card.Title>New token</Card.Title>
			<Card.Description>
				A token acts as you, on every map you can reach, with the role you hold there. Name it after
				the script that will use it.
			</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col gap-4">
			<form class="flex flex-wrap items-end gap-2" onsubmit={create}>
				<div class="flex min-w-48 flex-1 flex-col gap-1.5">
					<Label for="token-name">Name</Label>
					<Input
						id="token-name"
						bind:value={name}
						placeholder="Fleet intel bot"
						maxlength={255}
						required
						data-testid="token-name"
					/>
				</div>
				<div class="flex flex-col gap-1.5">
					<Label for="token-lifetime">Lifetime</Label>
					<Select.Root type="single" bind:value={lifetime}>
						<Select.Trigger id="token-lifetime" class="w-40" data-testid="token-lifetime">
							{lifetimes.find((l) => l.value === lifetime)?.label}
						</Select.Trigger>
						<Select.Content>
							{#each lifetimes as option (option.value)}
								<Select.Item value={option.value} label={option.label} />
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<Button type="submit" disabled={creating || !name.trim()} data-testid="token-create">
					Create
				</Button>
			</form>

			{#if created}
				<div
					class="flex flex-col gap-2 border border-amber-500/40 bg-amber-500/5 p-3"
					data-testid="token-created"
				>
					<p class="text-sm">
						<span class="font-medium">{created.name}</span> is ready. Copy it now: it is not shown again.
					</p>
					<div class="flex items-center gap-2">
						<code
							class="min-w-0 flex-1 overflow-x-auto bg-background px-2 py-1.5 font-mono text-xs"
							data-testid="token-plaintext">{created.plaintext}</code
						>
						<Button
							variant="outline"
							size="icon"
							aria-label="Copy token"
							onclick={() => created && copyText(created.plaintext, { success: 'Token copied' })}
							data-testid="token-copy"
						>
							<CopyIcon class="size-4" />
						</Button>
					</div>
				</div>
			{/if}
		</Card.Content>
	</Card.Root>

	<Card.Root>
		<Card.Header>
			<Card.Title>Your tokens</Card.Title>
			<Card.Description>
				Revoking one locks its script out on the next request. Tokens cannot make or remove other
				tokens: that only happens here.
			</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col divide-y divide-border/40">
			{#each tokens as token (token.id)}
				<div
					class="flex items-center gap-3 py-3"
					data-testid="token-row"
					data-token-name={token.name}
				>
					<span class="flex min-w-0 flex-1 flex-col">
						<span class="truncate text-sm font-medium">{token.name}</span>
						<span class="text-xs text-muted-foreground">
							{token.last_used_at ? `Last used ${timeAgo(token.last_used_at)}` : 'Never used'}
							<span aria-hidden="true">·</span>
							{expiry(token)}
						</span>
					</span>
					<Button
						variant="ghost"
						size="icon"
						class="size-8 text-muted-foreground hover:text-destructive"
						aria-label="Revoke {token.name}"
						onclick={() => revoke(token)}
						data-testid="token-revoke"
					>
						<TrashIcon />
					</Button>
				</div>
			{/each}
			{#if !tokensQuery.isPending && tokens.length === 0}
				<p class="py-4 text-sm text-muted-foreground" data-testid="tokens-empty">No tokens yet.</p>
			{/if}
		</Card.Content>
	</Card.Root>

	<Card.Root>
		<Card.Header>
			<Card.Title>Using one</Card.Title>
			<Card.Description>Send it as a bearer header on any request.</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col gap-3">
			<pre
				class="overflow-x-auto bg-muted px-3 py-2 font-mono text-xs">curl -H "Authorization: Bearer wst_..." {page
					.url.origin}/api/maps</pre>
			<p class="text-sm text-muted-foreground">
				Everything the map screen does is open to a token. The full reference is at
				<a href="/api/docs" class="underline" target="_blank" rel="noopener">/api/docs</a>; see also
				<a href="/documentation/contributing-and-self-hosting/using-the-api" class="underline"
					>Using the API</a
				>.
			</p>
		</Card.Content>
	</Card.Root>
</div>
