<script lang="ts">
	// The System Info card for the active system (legacy SystemInfo port): hero with class,
	// alias, effect, shattered badge, occupier and region; external links; statics with
	// physics popovers; effect modifiers; sovereignty.
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';

	import { api } from '$lib/api/client';
	import type { EffectModifier } from '$lib/api/types/EffectModifier';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Popover from '$lib/components/ui/popover';
	import EveImage from '$lib/components/EveImage.svelte';
	import StaticDetails from '$lib/components/map/StaticDetails.svelte';
	import { classMeta, destClassMeta, isWormholeClass } from '$lib/map/classes';

	let { system }: { system: MapSystemView } = $props();

	const cls = $derived(classMeta(system.wormhole_class_id, system.security_status));
	const isWormhole = $derived(isWormholeClass(system.wormhole_class_id));
	const underscore = (s: string) => s.replaceAll(' ', '_');
	const effectToken = $derived(system.effect_name?.toLowerCase().replaceAll(' ', '-'));
	const sovKind = $derived(system.sovereignty?.kind === 'alliance' ? 'alliance' : 'corporation');

	let mods = $state<EffectModifier[]>([]);
	$effect(() => {
		mods = [];
		if (system.effect_name) {
			api
				.effectModifiers(system.effect_name, system.wormhole_class_id ?? 0)
				.then((m) => (mods = m))
				.catch(() => {});
		}
	});

	const dotlanUrl = $derived(
		isWormhole
			? `https://evemaps.dotlan.net/system/${underscore(system.name)}`
			: `https://evemaps.dotlan.net/map/${underscore(system.region)}/${underscore(system.name)}`
	);
</script>

<Card.Root data-testid="system-info">
	<Card.Header>
		<Card.Title class="flex items-center gap-2">
			<span class="font-medium" style="color: var(--color-{cls.token})">{cls.short}</span>
			{#if system.alias}
				{system.alias}
				<span class="text-xs font-normal text-muted-foreground">({system.name})</span>
			{:else}
				{system.name}
			{/if}
			{#if system.is_shattered}
				<Badge variant="outline" class="text-amber-500">Shattered</Badge>
			{/if}
		</Card.Title>
		<Card.Description class="flex flex-col gap-0.5">
			{#if system.effect_name}
				<span style="color: var(--color-{effectToken})">{system.effect_name}</span>
			{/if}
			{#if system.occupying_group}
				<span>Occupied by {system.occupying_group}</span>
			{/if}
			<span>{system.region} · {system.constellation}</span>
		</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-col gap-3 text-xs">
		<div class="flex gap-3">
			<a
				class="text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
				href="https://zkillboard.com/system/{system.solar_system_id}/"
				target="_blank"
				rel="noopener">zKill</a
			>
			<a
				class="text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
				href={dotlanUrl}
				target="_blank"
				rel="noopener">Dotlan</a
			>
			{#if isWormhole}
				<a
					class="text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
					href="https://anoik.is/systems/{system.name}"
					target="_blank"
					rel="noopener">Anoik</a
				>
			{/if}
		</div>

		{#if system.statics.length > 0}
			<div class="flex flex-wrap gap-1.5">
				{#each system.statics as st (st.code)}
					{@const dest = destClassMeta(st.dest_class)}
					<Popover.Root>
						<Popover.Trigger
							class="flex items-center gap-1 border border-border px-1.5 py-0.5 hover:bg-accent"
						>
							{st.code}
							<span style="color: var(--color-{dest.token})">{dest.short}</span>
						</Popover.Trigger>
						<Popover.Content class="w-auto p-0">
							<StaticDetails static={st} />
						</Popover.Content>
					</Popover.Root>
				{/each}
			</div>
		{/if}

		{#if mods.length > 0}
			<div class="flex flex-col gap-0.5">
				{#each mods as m (m.stat + m.kind)}
					<div class="flex items-center justify-between gap-2">
						<span class="text-muted-foreground">{m.stat}</span>
						<span class="flex items-center gap-0.5">
							{m.value}
							{#if m.kind === 'buff'}
								<ArrowUpIcon class="size-3 text-green-500" />
							{:else}
								<ArrowDownIcon class="size-3 text-red-500" />
							{/if}
						</span>
					</div>
				{/each}
			</div>
		{/if}

		{#if system.sovereignty}
			<div class="flex items-center gap-2">
				<EveImage kind={sovKind} id={system.sovereignty.id} class="size-6 rounded-sm" />
				<span>
					{#if 'ticker' in system.sovereignty}[{system.sovereignty.ticker}]{/if}
					{system.sovereignty.name}
				</span>
			</div>
		{/if}
	</Card.Content>
</Card.Root>
