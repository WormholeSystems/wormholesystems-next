<script lang="ts">
	// The System Info panel for the active system (legacy SystemInfo port): hero with class,
	// alias, effect, shattered chip, occupier and region/links line; then bordered blocks
	// for statics (physics popovers), effect modifiers, and sovereignty.
	import { api } from '$lib/api/client';
	import type { EffectModifier } from '$lib/api/types/EffectModifier';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import * as Popover from '$lib/components/ui/popover';
	import EveImage from '$lib/components/EveImage.svelte';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import StaticDetails from '$lib/components/map/StaticDetails.svelte';
	import { classMeta, destClassMeta, effectTextColor, isWormholeClass } from '$lib/map/classes';

	let { system }: { system: MapSystemView } = $props();

	const cls = $derived(classMeta(system.wormhole_class_id, system.security_status));
	const isWormhole = $derived(isWormholeClass(system.wormhole_class_id));
	const underscore = (s: string) => s.replaceAll(' ', '_');

	const effectColor = $derived(effectTextColor(system.effect_name));

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

<MapPanel testid="system-info">
	<MapPanelHeader>System</MapPanelHeader>
	<MapPanelContent>
		<div class="border-b border-border/50 px-3 py-3">
			<div class="flex items-center gap-2">
				<span style:color="var(--color-{cls.token})">{cls.short}</span>
				<span class="truncate text-sm font-medium">
					{#if system.alias}
						{system.alias} <span class="text-muted-foreground">({system.name})</span>
					{:else}
						{system.name}
					{/if}
				</span>
				{#if system.effect_name}
					<span class="shrink-0 text-[10px] {effectColor}">{system.effect_name}</span>
				{/if}
				{#if system.is_shattered}
					<span class="shrink-0 text-[10px] text-amber-500">Shattered</span>
				{/if}
			</div>
			{#if system.occupying_group}
				<div class="mt-1 text-[11px] text-muted-foreground">
					Occupied by <span class="font-medium text-foreground">{system.occupying_group}</span>
				</div>
			{/if}
			<div class="mt-1 flex items-center gap-1 text-[11px] text-muted-foreground">
				<span>{system.region}</span>
				{#if system.constellation}
					<span>· {system.constellation}</span>
				{/if}
				<span class="text-border">·</span>
				<a
					class="transition-colors hover:text-foreground"
					href="https://zkillboard.com/system/{system.solar_system_id}/"
					target="_blank"
					rel="noopener">zKill</a
				>
				<a class="transition-colors hover:text-foreground" href={dotlanUrl} target="_blank" rel="noopener"
					>Dotlan</a
				>
				{#if isWormhole}
					<a
						class="transition-colors hover:text-foreground"
						href="https://anoik.is/systems/{system.name}"
						target="_blank"
						rel="noopener">Anoik</a
					>
				{/if}
			</div>
		</div>

		{#if system.statics.length > 0}
			<div class="border-b border-border/50 px-3 py-2">
				<div class="flex items-center gap-2">
					<span class="text-[10px] tracking-wider text-muted-foreground uppercase">Statics</span>
					<div class="flex gap-1.5">
						{#each system.statics as st (st.code)}
							{@const dest = destClassMeta(st.dest_class)}
							<Popover.Root>
								<Popover.Trigger
									class="font-mono text-xs font-medium transition-opacity hover:opacity-70"
									style="color: var(--color-{dest.token})"
								>
									{st.code} <span class="uppercase opacity-60">{dest.short}</span>
								</Popover.Trigger>
								<Popover.Content class="w-48 p-0" align="start">
									<StaticDetails static={st} />
								</Popover.Content>
							</Popover.Root>
						{/each}
					</div>
				</div>
			</div>
		{/if}

		{#if mods.length > 0}
			<div class="border-b border-border/50 px-3 py-2">
				<div class="flex flex-col gap-1">
					<span class="text-[10px] tracking-wider text-muted-foreground uppercase">Effect</span>
					<div class="grid grid-cols-2 gap-x-4 gap-y-0.5">
						{#each mods as m (m.stat + m.kind)}
							<div class="flex items-center justify-between text-[11px]">
								<span class="truncate text-muted-foreground">{m.stat}</span>
								<span class={m.kind === 'buff' ? 'text-green-400' : 'text-red-400'}>{m.value}</span>
							</div>
						{/each}
					</div>
				</div>
			</div>
		{/if}

		{#if system.sovereignty}
			<div class="px-3 py-2">
				<div class="flex flex-col gap-1.5">
					<span class="text-[10px] tracking-wider text-muted-foreground uppercase">Sovereignty</span>
					<div class="flex items-center gap-2">
						<EveImage kind={system.sovereignty.kind} id={system.sovereignty.id} size={32} class="size-5 rounded" />
						<span class="text-xs">
							{#if 'ticker' in system.sovereignty}<span class="font-medium"
									>[{system.sovereignty.ticker}]</span
								>
							{/if}{system.sovereignty.name}
						</span>
					</div>
				</div>
			</div>
		{/if}
	</MapPanelContent>
</MapPanel>
