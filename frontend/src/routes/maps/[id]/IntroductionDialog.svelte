<script lang="ts">
	// The one-time walkthrough a map opens with, ported from legacy's MapIntroduction:
	// welcome, the ESI permissions, the preferences that depend on them, and a summary.
	//
	// The reason it is a wizard where the setup guide is a checklist: permissions are the
	// one part of the setup a map cannot infer or do for you. A scope is granted in EVE's
	// SSO, off-site, and until it is nothing else on this screen works. So it gets a step
	// of its own that states plainly what each permission buys and what it costs.
	//
	// Two things it does that legacy does not. Granting one permission asks for everything
	// already consented to as well, because SSO reissues the token wholesale and legacy's
	// per-scope link silently dropped the rest. And the settings step disables what the
	// missing scopes cannot support, rather than offering switches that quietly do nothing.
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import CheckCircleIcon from '@lucide/svelte/icons/check-circle-2';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import MapPinIcon from '@lucide/svelte/icons/map-pin';
	import RouteIcon from '@lucide/svelte/icons/route';
	import ShieldIcon from '@lucide/svelte/icons/shield';
	import SignatureIcon from '@lucide/svelte/icons/scan-line';
	import TagIcon from '@lucide/svelte/icons/tag';
	import ZapIcon from '@lucide/svelte/icons/zap';

	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { ScopeStatus } from '$lib/api/types/ScopeStatus';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Switch } from '$lib/components/ui/switch';
	import { cn } from '$lib/utils';
	import type { MapState } from './map-state.svelte';

	let { map }: { map: MapState } = $props();

	const STEPS = [
		{
			title: 'Welcome to the map',
			blurb: 'A minute of setup, and it maps the chain as you fly it.'
		},
		{
			title: 'Grant permissions',
			blurb: 'What the map may read from your EVE client. All optional.'
		},
		{ title: 'Choose what it does', blurb: 'How much of the mapping you want done for you.' },
		{ title: 'Ready to fly', blurb: 'Here is where everything ended up.' }
	];

	// Copy for each ESI scope: what it is called in EVE, and what it actually buys here.
	const SCOPES = [
		{
			scope: 'esi-location.read_location.v1',
			name: 'Character location',
			body:
				'Where you are. Puts you on your system for everyone on the map, and measures ' +
				'every distance from where you actually are.',
			icon: MapPinIcon
		},
		{
			scope: 'esi-location.read_online.v1',
			name: 'Online status',
			body:
				'Whether you are logged in, so the map stops reporting you as somewhere you left ' +
				'hours ago.',
			icon: ZapIcon
		},
		{
			scope: 'esi-location.read_ship_type.v1',
			name: 'Ship type',
			body:
				'What you are flying. The difference between "someone is in the hole" and ' +
				'"a Loki is in the hole".',
			icon: ShieldIcon
		},
		{
			scope: 'esi-ui.write_waypoint.v1',
			name: 'Set waypoints',
			body:
				'Lets the map put a destination straight into your client, instead of you retyping ' +
				'system names.',
			icon: RouteIcon
		}
	];

	let step = $state(1);
	let scopes = $state<ScopeStatus[]>([]);

	$effect(() => {
		api
			.myScopes()
			.then((rows) => (scopes = rows))
			.catch(() => {});
	});

	const granted = $derived(new Set(scopes.filter((s) => s.granted).map((s) => s.scope)));
	const missing = $derived(SCOPES.filter((s) => !granted.has(s.scope)));
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
			return_to: page.url.pathname + page.url.search
		});
		return `/auth/login?${params}`;
	}

	function update(patch: Record<string, boolean>) {
		api
			.updateMapUserSettings(map.mapId, patch)
			.then((s) => {
				map.userSettings = s;
				if ('tracking_allowed' in patch) map.fetchCharacters();
			})
			.catch((err) => (map.statusLine = `setup: ${(err as Error).message}`));
	}

	function finish() {
		closed = true;
		update({ introduction_confirmed: true });
	}

	const summary = $derived([
		{
			label: 'Permissions',
			value:
				missing.length === 0
					? 'All granted'
					: `${SCOPES.length - missing.length} of ${SCOPES.length} granted`,
			good: missing.length === 0
		},
		{
			label: 'Location sharing',
			value: settings?.tracking_allowed ? 'On' : 'Off',
			good: settings?.tracking_allowed ?? false
		},
		{
			label: 'Signature prompt',
			value: settings?.tracking_allowed && settings?.prompt_for_signature ? 'On' : 'Off',
			good: (settings?.tracking_allowed && settings?.prompt_for_signature) ?? false
		}
	]);

	const opening = [
		{ icon: ShieldIcon, text: 'The EVE permissions the map can use' },
		{ icon: EyeIcon, text: 'Whether it may follow you around' },
		{ icon: RouteIcon, text: 'How much of the mapping it does for you' }
	];

	const toggles = $derived([
		{
			key: 'tracking_allowed',
			icon: EyeIcon,
			name: 'Share my location on this map',
			body:
				'The map follows you between systems, shows you to everyone else here, and measures ' +
				'distances from where you are. Revocable at any time.',
			value: settings?.tracking_allowed ?? false,
			enabled: hasLocation,
			blocked: 'Needs the character location permission.'
		},
		{
			key: 'prompt_for_signature',
			icon: SignatureIcon,
			name: 'Ask which signature I jumped',
			body:
				'When you arrive somewhere new, the map asks which signature the hole was and links ' +
				'it, instead of leaving an unnamed connection behind.',
			value: settings?.prompt_for_signature ?? true,
			enabled: settings?.tracking_allowed ?? false,
			blocked: 'Needs location sharing.'
		},
		{
			key: 'suggest_alias',
			icon: TagIcon,
			name: 'Name new systems for me',
			body:
				"Fills in the next alias from the chain's naming scheme, so holes are named the " +
				'same way by everyone.',
			value: settings?.suggest_alias ?? true,
			enabled: settings?.tracking_allowed ?? false,
			blocked: 'Needs location sharing.'
		}
	]);
</script>

<Dialog.Root {open} onOpenChange={(v) => !v && open && finish()}>
	<Dialog.Content
		class="max-h-[90vh] w-full overflow-y-auto md:max-w-2xl"
		data-testid="introduction"
	>
		<Dialog.Header>
			<Dialog.Title class="font-heading text-lg">{STEPS[step - 1].title}</Dialog.Title>
			<Dialog.Description>{STEPS[step - 1].blurb}</Dialog.Description>
		</Dialog.Header>

		<div class="flex items-center gap-1.5" data-testid="introduction-progress">
			{#each STEPS as _, i (i)}
				<div class={cn('h-1 flex-1', i < step ? 'bg-primary' : 'bg-muted')}></div>
			{/each}
			<span class="ml-1 font-mono text-[10px] tabular-nums text-muted-foreground">
				{step}/{STEPS.length}
			</span>
		</div>

		{#if step === 1}
			<div class="flex flex-col gap-4 text-sm">
				<p class="text-muted-foreground">
					Vector keeps a wormhole chain that several people edit at once. It can also build
					that chain from where your characters actually are, which is the part worth setting
					up now.
				</p>
				<div class="flex flex-col gap-2 border border-border/60 p-3">
					{#each opening as row, i (i)}
						{@const Icon = row.icon}
						<span class="flex items-center gap-2 text-xs">
							<Icon class="size-3.5 text-muted-foreground" />
							{row.text}
						</span>
					{/each}
				</div>
				<p class="text-xs text-muted-foreground">
					Nothing here is permanent. Every one of these can be changed later in the map's
					settings.
				</p>
			</div>
		{:else if step === 2}
			<div class="flex flex-col divide-y divide-border/60">
				{#each SCOPES as item (item.scope)}
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
						Paste a signature scan anywhere on the map to fill in a system. Right-click a
						system for its menu, and drag between two to connect them.
					</p>
					<p>
						Everything here lives in the map's settings afterwards, along with access for the
						rest of your corp.
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
				{#if step < STEPS.length}
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
