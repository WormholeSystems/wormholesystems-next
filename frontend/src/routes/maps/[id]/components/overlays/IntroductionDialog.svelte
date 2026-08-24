<script lang="ts">
	// The one-time walkthrough a map opens with: welcome, the ESI permissions, the preferences
	// that depend on them, and a summary.
	//
	// Granting one permission asks for everything already consented to as well, because SSO
	// reissues the token wholesale and a per-scope link would drop the rest. The settings step
	// disables what the missing scopes cannot support rather than offering dead switches.
	import { api, errorMessage } from '$lib/api/client';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import CheckCircleIcon from '@lucide/svelte/icons/check-circle-2';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';

	import { createQuery } from '@tanstack/svelte-query';
	import { page } from '$app/state';
	import { toast } from 'svelte-sonner';

	import { q } from '$lib/api/queries';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Switch } from '$lib/components/ui/switch';
	import { cn } from '$lib/utils';
	import type { MapState } from '../../state/map-state.svelte';
	import { atLeast } from '$lib/map/roles';
	import { ESI_SCOPES } from '$lib/esi/scopes';
	import { INTRO_OPENING, INTRO_STEPS, introSummary, introToggles } from './introduction-content';
	import { PLACEMENTS } from '$lib/map/placement';

	let { map }: { map: MapState } = $props();

	let step = $state(1);
	const scopesQuery = createQuery(() => ({ ...q.myScopes(), enabled: map.signedIn }));
	const scopes = $derived(scopesQuery.data ?? []);

	const granted = $derived(new Set(scopes.filter((s) => s.granted).map((s) => s.scope)));
	const missing = $derived(ESI_SCOPES.filter((s) => !granted.has(s.scope)));
	const hasLocation = $derived(granted.has('esi-location.read_location.v1'));

	const settings = $derived(map.userSettings);
	// Closed locally the moment it is dismissed, rather than waiting for the settings round
	// trip: without that the dialog is still open when the close handler fires again.
	let closed = $state(false);
	const open = $derived(!closed && !(settings?.introduction_confirmed ?? true));

	/** Consent for these on top of everything already granted, then come back here. */
	function grantUrl(wanted: string[]) {
		const params = new URLSearchParams({
			scopes: wanted.join(','),
			return_to: page.url.pathname + page.url.search,
		});
		return `/auth/login?${params}`;
	}

	// Placement is the map's, not this viewer's, so only a manager is offered it.
	const canManage = $derived(atLeast(map.data?.role, 'manager'));
	const placement = $derived(map.data?.map.layout === 'tree' ? 'tree' : 'manual');
	function setPlacement(layout: 'manual' | 'tree') {
		map.run('setPlacement', api.updateMap({ map_id: map.mapId, layout }));
	}

	function update(patch: Record<string, boolean>) {
		map
			.patchUserSettings(patch)
			.then(() => {
				if ('tracking_allowed' in patch) map.refreshCharacters();
			})
			.catch((err) => toast.error(`setup: ${errorMessage(err)}`));
	}

	function finish() {
		closed = true;
		update({ introduction_confirmed: true });
	}

	const summary = $derived(
		introSummary(settings, ESI_SCOPES.length - missing.length, ESI_SCOPES.length),
	);

	const toggles = $derived(introToggles(settings, hasLocation));
</script>

<Dialog.Root {open} onOpenChange={(v) => !v && open && finish()}>
	<Dialog.Content
		class="max-h-[90vh] w-full overflow-y-auto md:max-w-2xl"
		data-testid="introduction"
	>
		<Dialog.Header>
			<Dialog.Title class="font-heading text-lg">{INTRO_STEPS[step - 1].title}</Dialog.Title>
			<Dialog.Description>{INTRO_STEPS[step - 1].blurb}</Dialog.Description>
		</Dialog.Header>

		<div class="flex items-center gap-1.5" data-testid="introduction-progress">
			{#each INTRO_STEPS as _, i (i)}
				<div class={cn('h-1 flex-1', i < step ? 'bg-primary' : 'bg-muted')}></div>
			{/each}
			<span class="ml-1 font-mono text-[10px] tabular-nums text-muted-foreground">
				{step}/{INTRO_STEPS.length}
			</span>
		</div>

		{#if step === 1}
			<div class="flex flex-col gap-4 text-sm">
				<p class="text-muted-foreground">
					WormholeSystems keeps a wormhole chain that several people edit at once. It can also build
					that chain from where your characters actually are, which is the part worth setting up
					now.
				</p>
				<div class="flex flex-col gap-2 border border-border/60 p-3">
					{#each INTRO_OPENING as row, i (i)}
						{@const Icon = row.icon}
						<span class="flex items-center gap-2 text-xs">
							<Icon class="size-3.5 text-muted-foreground" />
							{row.text}
						</span>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					Nothing here is permanent. Every one of these can be changed later in the map's settings.
				</p>
			</div>
		{:else if step === 2}
			<div class="flex flex-col divide-y divide-border/60">
				{#each ESI_SCOPES as item (item.scope)}
					{@const Icon = item.icon}
					{@const ok = granted.has(item.scope)}
					<div class="flex items-start justify-between gap-3 py-3" data-scope={item.scope}>
						<span class="flex items-start gap-2.5">
							<Icon class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
							<span>
								<span class="text-sm font-medium">{item.name}</span>
								<p class="mt-0.5 text-xs leading-relaxed text-muted-foreground">{item.body}</p>
							</span>
						</span>
						{#if ok}
							<span
								class="flex shrink-0 items-center gap-1 text-xs text-emerald-500"
								data-testid="scope-granted"
							>
								<CheckCircleIcon class="size-3.5" />
								Granted
							</span>
						{:else}
							<Button variant="outline" size="sm" href={grantUrl([item.scope])} class="shrink-0">
								<ExternalLinkIcon data-icon="inline-start" />
								Grant
							</Button>
						{/if}
					</div>
				{/each}
			</div>
		{:else if step === 3}
			<div class="flex flex-col gap-3">
				{#if canManage}
					<div class="flex flex-col gap-2" data-testid="setup-placement">
						<span class="text-xs text-muted-foreground">
							How the chain is laid out, for everyone on this map. Changeable later in the map's
							settings.
						</span>
						<div class="grid grid-cols-2 gap-2">
							{#each PLACEMENTS as option (option.value)}
								{@const Icon = option.icon}
								{@const chosen = placement === option.value}
								<button
									type="button"
									class={cn(
										'flex flex-col gap-1 border p-3 text-left transition-colors',
										chosen
											? 'border-primary/60 bg-accent/40'
											: 'border-border/60 hover:bg-accent/20',
									)}
									aria-pressed={chosen}
									data-testid="setup-placement-{option.value}"
									onclick={() => setPlacement(option.value)}
								>
									<span class="flex items-center gap-1.5 text-sm font-medium">
										<Icon class={cn('size-4', chosen ? 'text-primary' : 'text-muted-foreground')} />
										{option.label}
									</span>
									<span class="text-xs leading-relaxed text-muted-foreground">{option.body}</span>
								</button>
							{/each}
						</div>
					</div>
				{/if}

				{#each toggles as row (row.key)}
					{@const Icon = row.icon}
					<div
						class={cn('border border-border/60 p-3', !row.enabled && 'opacity-60')}
						data-setting={row.key}
					>
						<div class="flex items-start justify-between gap-3">
							<span class="flex items-start gap-2.5">
								<Icon class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
								<span>
									<span class="text-sm font-medium">{row.name}</span>
									<p class="mt-0.5 text-xs leading-relaxed text-muted-foreground">{row.body}</p>
									{#if !row.enabled}
										<p class="mt-1 text-xs text-amber-500">{row.blocked}</p>
									{/if}
								</span>
							</span>
							<Switch
								checked={row.value && row.enabled}
								disabled={!row.enabled}
								aria-label={row.name}
								onCheckedChange={(v) => update({ [row.key]: v })}
							/>
						</div>
					</div>
				{/each}
			</div>
		{:else}
			<div class="flex flex-col gap-4">
				<div class="flex flex-col gap-1.5 border border-border/60 p-3 text-xs">
					{#each summary as row (row.label)}
						<span class="flex items-center justify-between">
							<span class="text-muted-foreground">{row.label}</span>
							<span class={row.good ? 'text-emerald-500' : 'text-amber-500'}>{row.value}</span>
						</span>
					{/each}
				</div>
				<div class="flex flex-col gap-2 text-xs text-muted-foreground">
					<p>
						Paste a signature scan anywhere on the map to fill in a system. Right-click a system for
						its menu, and drag between two to connect them.
					</p>
					<p>
						Everything here lives in the map's settings afterwards, along with access for the rest
						of your corp.
					</p>
				</div>
			</div>
		{/if}

		<div class="flex items-center justify-between pt-2">
			<Button
				variant="outline"
				size="sm"
				disabled={step === 1}
				onclick={() => (step -= 1)}
				data-testid="introduction-back"
			>
				<ArrowLeftIcon data-icon="inline-start" />
				Back
			</Button>
			<div class="flex gap-2">
				{#if step === 2 && missing.length > 0}
					<Button size="sm" href={grantUrl(missing.map((s) => s.scope))}>Grant all</Button>
				{/if}
				{#if step < INTRO_STEPS.length}
					<Button
						size="sm"
						variant={step === 2 && missing.length > 0 ? 'outline' : 'default'}
						onclick={() => (step += 1)}
						data-testid="introduction-next"
					>
						{step === 2 && missing.length > 0 ? 'Skip for now' : 'Next'}
						<ArrowRightIcon data-icon="inline-end" />
					</Button>
				{:else}
					<Button size="sm" onclick={finish} data-testid="introduction-done">Start mapping</Button>
				{/if}
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>
