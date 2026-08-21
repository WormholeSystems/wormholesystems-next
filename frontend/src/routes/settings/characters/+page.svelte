<script lang="ts">
	// The characters signed in to this account. Permissions live here rather than on a map:
	// they are granted per character at the EVE SSO and apply everywhere. Adding one is the
	// same login flow with `?link=true`.
	import CheckCircleIcon from '@lucide/svelte/icons/check-circle-2';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import StarIcon from '@lucide/svelte/icons/star';
	import TrashIcon from '@lucide/svelte/icons/trash-2';
	import { lookup } from '$lib/enums';

	import { api, errorMessage } from '$lib/api/client';
	import type { CharacterRef } from '$lib/api/types/CharacterRef';
	import type { ScopeStatus } from '$lib/api/types/ScopeStatus';
	import EveImage from '$lib/components/EveImage.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { cn } from '$lib/utils';

	let characters = $state<CharacterRef[]>([]);
	let scopes = $state<ScopeStatus[]>([]);
	let error = $state<string | null>(null);

	const SCOPES = {
		'esi-location.read_location.v1': {
			name: 'Character location',
			blurb: 'Puts you on your system, and measures distances from where you are.',
		},
		'esi-location.read_online.v1': {
			name: 'Online status',
			blurb: 'Stops the map reporting you as somewhere you left hours ago.',
		},
		'esi-location.read_ship_type.v1': {
			name: 'Ship type',
			blurb: 'Shows what you are flying, not just that you are there.',
		},
		'esi-ui.write_waypoint.v1': {
			name: 'Set waypoints',
			blurb: 'Lets the map put a destination straight into your client.',
		},
	} satisfies Record<string, { name: string; blurb: string }>;

	async function load() {
		try {
			[characters, scopes] = await Promise.all([api.myCharacters(), api.myScopes()]);
			error = null;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	$effect(() => {
		load();
	});

	async function act(work: Promise<unknown>) {
		try {
			await work;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function remove(character: CharacterRef) {
		if (!confirm(`Remove ${character.name} from this account?`)) return;
		act(api.removeCharacter(character.character_id));
	}

	const missing = $derived(scopes.filter((s) => !s.granted));
	const grantUrl = $derived(
		`/auth/login?scopes=${encodeURIComponent(missing.map((s) => s.scope).join(','))}&return_to=${encodeURIComponent('/settings/characters')}`,
	);
</script>

<div class="flex flex-col gap-6">
	{#if error}
		<p class="text-sm text-destructive" data-testid="characters-error">{error}</p>
	{/if}

	<Card.Root>
		<Card.Header>
			<div class="flex items-start justify-between gap-3">
				<div class="flex flex-col gap-1.5">
					<Card.Title>Characters</Card.Title>
					<Card.Description>
						The active one is who the map acts as. Switching is instant and changes nothing else.
						The starred one is who you start as when you sign in on a new device.
					</Card.Description>
				</div>
				<Button
					variant="outline"
					size="sm"
					href="/auth/login?link=true"
					data-testid="add-character"
				>
					<PlusIcon data-icon="inline-start" />
					Add character
				</Button>
			</div>
		</Card.Header>
		<Card.Content class="flex flex-col divide-y divide-border/40">
			{#each characters as character (character.character_id)}
				<div
					class={cn('flex items-center gap-3 py-3', character.is_active && 'font-medium')}
					data-testid="character-row"
					data-character={character.character_id}
				>
					<EveImage
						kind="character"
						id={character.character_id}
						size={64}
						title={character.name}
						class="size-9 shrink-0 rounded-sm"
					/>
					<span class="flex min-w-0 flex-1 flex-col">
						<span class="truncate text-sm">{character.name}</span>
						<span class="text-xs text-muted-foreground">
							{character.online ? 'Online' : 'Offline'}
						</span>
					</span>
					{#if character.is_preferred}
						<span
							class="flex size-8 shrink-0 items-center justify-center text-amber-400"
							title="New sessions start as this character"
							data-testid="preferred-character"
						>
							<StarIcon class="size-4 fill-current" />
						</span>
					{:else}
						<Button
							variant="ghost"
							size="icon"
							class="size-8 text-muted-foreground/50 hover:text-foreground"
							aria-label="Start new sessions as {character.name}"
							onclick={() => act(api.setPreferredCharacter(character.character_id))}
							data-testid="prefer-character"
						>
							<StarIcon class="size-4" />
						</Button>
					{/if}
					{#if character.is_active}
						<Badge variant="outline" class="gap-1 shrink-0">
							<CheckCircleIcon />
							Acting
						</Badge>
					{:else}
						<Button
							variant="ghost"
							size="sm"
							onclick={() => act(api.switchCharacter(character.character_id))}
							data-testid="switch-character">Act as</Button
						>
						<Button
							variant="ghost"
							size="icon"
							class="size-8 text-muted-foreground hover:text-destructive"
							aria-label="Remove {character.name}"
							onclick={() => remove(character)}
						>
							<TrashIcon />
						</Button>
					{/if}
				</div>
			{/each}
			{#if characters.length === 0}
				<p class="py-4 text-sm text-muted-foreground">No characters yet.</p>
			{/if}
		</Card.Content>
	</Card.Root>

	<Card.Root>
		<Card.Header>
			<Card.Title>EVE permissions</Card.Title>
			<Card.Description>
				What EVE lets WormholeSystems read for the acting character. All optional, and revocable in
				EVE's own settings at any time.
			</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col divide-y divide-border/40">
			{#each scopes as scope (scope.scope)}
				{@const meta = lookup(SCOPES, scope.scope)}
				<div class="flex items-start justify-between gap-4 py-3" data-testid="scope-row">
					<span class="flex min-w-0 flex-col gap-0.5">
						<span class="text-sm font-medium">{meta?.name ?? scope.scope}</span>
						{#if meta}
							<span class="text-xs text-muted-foreground">{meta.blurb}</span>
						{/if}
					</span>
					{#if scope.granted}
						<span
							class="flex shrink-0 items-center gap-1 text-xs text-emerald-500"
							data-testid="scope-granted"
						>
							<CheckCircleIcon class="size-3.5" />
							Granted
						</span>
					{:else}
						<span class="shrink-0 text-xs text-amber-500">Not granted</span>
					{/if}
				</div>
			{/each}
		</Card.Content>
		{#if missing.length > 0}
			<Card.Footer class="flex-col items-start gap-2">
				<Button href={grantUrl} size="sm" data-testid="grant-scopes">
					Grant the {missing.length} missing
					{missing.length === 1 ? 'permission' : 'permissions'}
				</Button>
				<p class="text-xs text-muted-foreground">
					This asks EVE for everything you have already granted as well, so nothing is lost by
					topping up.
				</p>
			</Card.Footer>
		{/if}
	</Card.Root>
</div>
