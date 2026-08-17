<script lang="ts">
	import LogOutIcon from '@lucide/svelte/icons/log-out';
	import MapIcon from '@lucide/svelte/icons/map';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';

	import { api } from '$lib/api/client';
	import type { CharacterRef } from '$lib/api/types/CharacterRef';
	import type { CharacterStatus } from '$lib/api/types/CharacterStatus';
	import type { CharacterSummary } from '$lib/api/types/CharacterSummary';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import EveImage from '$lib/components/EveImage.svelte';
	import ServerStatus from '$lib/components/ServerStatus.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { openUserSocket } from '$lib/ws';

	let { me }: { me: CharacterSummary | null } = $props();

	let characters = $state<CharacterRef[]>([]);
	let status = $state<CharacterStatus | null>(null);

	$effect(() => {
		if (!me) return;
		api.myCharacters().then((list) => (characters = list)).catch(() => {});
		const refetchStatus = () =>
			api
				.meStatus()
				.then((s) => (status = s))
				.catch(() => {});
		refetchStatus();
		// The user socket doubles as the activity heartbeat; each event means "your
		// status changed".
		return openUserSocket((event) => {
			if (event.type === 'character_status_changed') refetchStatus();
		});
	});

	async function switchCharacter(id: number) {
		await api.switchCharacter(id);
		location.reload();
	}

	async function removeCharacter(id: number) {
		await api.removeCharacter(id);
		location.reload();
	}
</script>

<nav class="sticky top-0 z-40 border-b border-border bg-background">
	<div class="flex h-12 items-center gap-6 px-5">
		<a href="/" class="font-heading text-sm font-semibold tracking-[0.2em] text-foreground">
			VECTOR
		</a>
		<a
			href="/maps"
			class="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
		>
			<MapIcon class="size-4" />
			Maps
		</a>

		<span class="ml-auto flex items-center gap-3">
			{#if status}
				<span class="hidden items-center gap-2 text-xs text-muted-foreground md:flex">
					<span
						class="size-1.5 rounded-full {status.online
							? 'bg-emerald-500'
							: 'bg-muted-foreground/40'}"
					></span>
					{#if status.ship_type_id != null}
						<EveImage kind="type" id={status.ship_type_id} class="size-4" />
					{/if}
					<span class="tracking-wide">{status.solar_system ?? '—'}</span>
				</span>
			{/if}
			<ServerStatus signedIn={me !== null} />
			<ThemeToggle />
			{#if me}
				<DropdownMenu.Root>
					<DropdownMenu.Trigger
						aria-label="Account"
						class="block size-7 overflow-hidden border border-border transition-colors hover:border-foreground/50"
					>
						<EveImage kind="character" id={me.character_id} class="size-7 object-cover" />
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="end">
						<DropdownMenu.Group>
							{#each characters as c (c.character_id)}
								<DropdownMenu.Item
									class={c.is_active ? 'text-foreground' : ''}
									onSelect={() => switchCharacter(c.character_id)}
								>
									<EveImage
										kind="character"
										id={c.character_id}
										class="size-5 border border-border"
									/>
									<span class="truncate">{c.name}</span>
								</DropdownMenu.Item>
							{/each}
						</DropdownMenu.Group>

						<DropdownMenu.Separator />

						<DropdownMenu.Group>
							<DropdownMenu.Item>
								{#snippet child({ props })}
									<a href="/auth/login?link=true" data-sveltekit-reload {...props}>
										<PlusIcon />
										Add character
									</a>
								{/snippet}
							</DropdownMenu.Item>

							{#if characters.length > 1}
								<DropdownMenu.Item
									variant="destructive"
									onSelect={() => removeCharacter(me.character_id)}
								>
									<Trash2Icon />
									Remove {me.name}
								</DropdownMenu.Item>
							{/if}
						</DropdownMenu.Group>

						<DropdownMenu.Separator />

						<DropdownMenu.Group>
							<DropdownMenu.Item>
								{#snippet child({ props })}
									<a href="/auth/logout" data-sveltekit-reload {...props}>
										<LogOutIcon />
										Log out
									</a>
								{/snippet}
							</DropdownMenu.Item>
						</DropdownMenu.Group>
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			{:else}
				<Button href="/auth/login" data-sveltekit-reload variant="outline">Log in</Button>
			{/if}
		</span>
	</div>
</nav>
