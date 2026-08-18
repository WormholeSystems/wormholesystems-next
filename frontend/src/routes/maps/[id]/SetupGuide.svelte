<script lang="ts">
	// What a new map still needs before it does anything useful.
	//
	// Deliberately not a wizard. Legacy's was a four-step modal you could not escape —
	// Escape, the overlay and the close button were all wired to nothing — and half of it
	// was a welcome screen and a feature grid that taught nothing you could act on.
	//
	// Instead every item is derived from the map's actual state, so it ticks itself off as
	// you work and can never claim you have done something you have not. It appears while
	// there is something left to do, it is dismissible for good, and each item does the
	// thing rather than telling you where to find it.
	import CheckIcon from '@lucide/svelte/icons/check';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import CrosshairIcon from '@lucide/svelte/icons/crosshair';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import UsersIcon from '@lucide/svelte/icons/users';
	import XIcon from '@lucide/svelte/icons/x';

	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { Button } from '$lib/components/ui/button';
	import { centerWorld, freePosition } from '$lib/map/helpers';
	import { cn } from '$lib/utils';
	import type { MapState } from './map-state.svelte';

	let { map }: { map: MapState } = $props();

	/** Where the acting pilot is, when we are allowed to know. */
	const pilot = $derived(map.myCharacters.find((c) => c.is_active && c.online) ?? null);
	const pilotSystem = $derived(pilot?.solar_system_id ?? null);
	let pilotSystemName = $state<string | null>(null);

	$effect(() => {
		const id = pilotSystem;
		pilotSystemName = null;
		if (id === null) return;
		api
			.resolveSystems([id])
			.then(([hit]) => (pilotSystemName = hit?.name ?? null))
			.catch(() => {});
	});

	const tracking = $derived(map.userSettings?.tracking_allowed ?? false);
	const hasSystems = $derived(map.systems.length > 0);

	// One grant is the map's own owner; a second means somebody else can see the chain.
	let memberCount = $state<number | null>(null);
	$effect(() => {
		const id = map.mapId;
		if (!id) return;
		api
			.listAccess(id)
			.then((rows) => (memberCount = rows.length))
			.catch(() => (memberCount = null));
	});

	interface Step {
		key: string;
		done: boolean;
		title: string;
		body: string;
		action: string;
		run: () => void;
	}

	const steps = $derived.by<Step[]>(() => [
		{
			key: 'home',
			done: hasSystems,
			title: 'Put your staging system on the map',
			body: pilotSystemName
				? `You are in ${pilotSystemName}. Start the chain there, or search for somewhere else.`
				: 'Everything hangs off the first system. Search for your staging system to place it.',
			action: pilotSystemName ? `Start at ${pilotSystemName}` : 'Add a system',
			run: () => (pilotSystem !== null ? placePilotSystem() : (map.searchOpen = true))
		},
		{
			key: 'tracking',
			done: tracking,
			title: 'Share your location on this map',
			// Concrete, not a privacy abstraction: what you get, and what others see. Careful
			// not to overclaim: your own client already knows where you are, so what is being
			// asked for is permission for *this map* to use and share it.
			body:
				'This map does not use your location yet. Turn it on and it builds the chain ' +
				'as you fly, puts you on your systems for everyone else here, and measures ' +
				'distances from where you actually are. Revocable at any time.',
			action: 'Share my location',
			run: enableTracking
		},
		{
			key: 'access',
			done: (memberCount ?? 1) > 1,
			title: 'Let your corp in',
			body: 'Right now only you can see this map. Access can be granted to a character, a corporation or a whole alliance.',
			action: 'Manage access',
			run: () => goto(`/maps/${map.mapId}/settings`)
		}
	]);

	const remaining = $derived(steps.filter((s) => !s.done).length);
	const dismissed = $derived(map.userSettings?.setup_dismissed ?? false);
	// The introduction is a modal over the whole map; the checklist waits its turn.
	const introduced = $derived(map.userSettings?.introduction_confirmed ?? false);

	// Open while there is work left and nobody has waved it away. The override holds the
	// open state directly, so the status-bar toggle can bring it back without having to
	// un-dismiss it in the database.
	let openOverride = $state<boolean | null>(null);
	const open = $derived(openOverride ?? (introduced && !dismissed && remaining > 0));

	export function toggle() {
		openOverride = !open;
	}

	function placePilotSystem() {
		if (pilotSystem === null) return;
		const base = centerWorld(map.pan, map.zoom, map.viewportRect());
		const at = freePosition(map.systems, base, map.grid);
		map.run(
			'add',
			api.addSystem({
				map_id: map.mapId,
				solar_system_id: pilotSystem,
				x: at.x,
				y: at.y,
				alias: null
			})
		);
	}

	function enableTracking() {
		api
			.updateMapUserSettings(map.mapId, { tracking_allowed: true })
			.then((s) => {
				map.userSettings = s;
				map.fetchCharacters();
			})
			.catch((err) => (map.statusLine = `tracking: ${(err as Error).message}`));
	}

	function dismiss() {
		openOverride = false;
		api
			.updateMapUserSettings(map.mapId, { setup_dismissed: true })
			.then((s) => (map.userSettings = s))
			.catch(() => {});
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- The card lives inside the canvas, whose pointerdown starts a marquee selection and
	     a pan. Without stopping here, pressing a button in the card begins a drag on the
	     map instead and the click never lands. -->
	<div
		class="absolute bottom-3 left-3 z-30 w-80 border border-border bg-card shadow-sm"
		data-testid="setup-guide"
		onpointerdown={(e) => e.stopPropagation()}
		oncontextmenu={(e) => e.stopPropagation()}
	>
		<div class="flex items-center justify-between border-b border-border/50 px-3 py-2">
			<span class="font-heading text-sm font-semibold">Set up this map</span>
			<span class="flex items-center gap-2">
				<span class="font-mono text-[10px] tabular-nums text-muted-foreground">
					{steps.length - remaining}/{steps.length}
				</span>
				<Button
					variant="ghost"
					size="icon"
					class="size-5"
					aria-label="Dismiss setup guide"
					onclick={dismiss}
					data-testid="setup-dismiss"
				>
					<XIcon />
				</Button>
			</span>
		</div>

		<div class="flex flex-col">
			{#each steps as step, i (step.key)}
				{@const Icon = [CrosshairIcon, EyeIcon, UsersIcon][i]}
				<div
					class="flex gap-2.5 border-b border-border/30 px-3 py-2.5 last:border-b-0"
					data-testid="setup-step"
					data-step={step.key}
					data-done={step.done}
				>
					<span
						class={cn(
							'mt-0.5 flex size-4 shrink-0 items-center justify-center border',
							step.done
								? 'border-emerald-500/40 bg-emerald-500/10 text-emerald-500'
								: 'border-border text-muted-foreground'
						)}
					>
						{#if step.done}
							<CheckIcon class="size-3" />
						{:else}
							<Icon class="size-2.5" />
						{/if}
					</span>

					<div class="min-w-0 flex-1">
						<p class={cn('text-xs font-medium', step.done && 'text-muted-foreground')}>
							{step.title}
						</p>
						{#if !step.done}
							<p class="mt-0.5 text-[11px] leading-relaxed text-muted-foreground">
								{step.body}
							</p>
							<button
								class="mt-1.5 inline-flex items-center gap-0.5 text-[11px] font-medium text-foreground hover:underline"
								onclick={step.run}
								data-testid="setup-action"
							>
								{step.action}
								<ChevronRightIcon class="size-3" />
							</button>
						{/if}
					</div>
				</div>
			{/each}
		</div>

		{#if remaining === 0}
			<div class="border-t border-border/50 px-3 py-2">
				<p class="text-[11px] text-muted-foreground">
					That is everything. Paste a signature scan into the map to start scanning.
				</p>
			</div>
		{/if}
	</div>
{/if}
