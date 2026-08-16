<script lang="ts">
	// The wormhole effect badge: a lettered colored circle (legacy palette); clicking it
	// opens a popover listing every modifier at this system's class.
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';

	import { api } from '$lib/api/client';
	import type { EffectModifier } from '$lib/api/types/EffectModifier';
	import * as Popover from '$lib/components/ui/popover';

	let { name, wormholeClassId }: { name: string; wormholeClassId: number } = $props();

	const token = $derived(name.toLowerCase().replaceAll(' ', '-'));
	const letter = $derived(name === 'Wolf-Rayet Star' ? 'W' : name.charAt(0).toUpperCase());
	const darkText = $derived(name === 'Cataclysmic Variable');

	let mods = $state<EffectModifier[]>([]);

	$effect(() => {
		api
			.effectModifiers(name, wormholeClassId)
			.then((m) => (mods = m))
			.catch(() => {});
	});
</script>

<Popover.Root>
	<Popover.Trigger
		aria-label={name}
		class="flex size-[14px] shrink-0 cursor-help items-center justify-center rounded-full text-[9px] font-semibold {darkText
			? 'text-neutral-950'
			: 'text-white'}"
		style="background-color: var(--color-{token})"
		onpointerdown={(ev: PointerEvent) => ev.stopPropagation()}
	>
		{letter}
	</Popover.Trigger>
	<Popover.Content class="w-56 p-2 text-[11px]" onpointerdown={(ev: PointerEvent) => ev.stopPropagation()}>
		<div class="mb-1 flex items-center gap-1.5 font-medium">
			<span
				class="flex size-[14px] items-center justify-center rounded-full text-[9px] font-semibold {darkText
					? 'text-neutral-950'
					: 'text-white'}"
				style:background-color="var(--color-{token})"
			>
				{letter}
			</span>
			{name}
		</div>
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
	</Popover.Content>
</Popover.Root>
