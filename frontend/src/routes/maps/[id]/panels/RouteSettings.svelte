<script lang="ts">
	// The route-calculation settings popover (legacy AutopilotSettings): preference,
	// security penalty, wormhole lifetime/mass tolerance, EVE Scout toggle. Stored per
	// user per map.
	import SlidersHorizontalIcon from '@lucide/svelte/icons/sliders-horizontal';
	import { Slider } from '$lib/components/ui/slider';

	import * as Popover from '$lib/components/ui/popover';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';
	import type { MapState } from '../map-state.svelte';

	let { map }: { map: MapState } = $props();

	const settings = $derived(map.userSettings);
	const preference = $derived(settings?.route_preference ?? 'shorter');

	const PREFS = [
		{ value: 'shorter', label: 'Shortest', hint: 'Min jumps' },
		{ value: 'safer', label: 'Safer', hint: 'High-sec' },
		{ value: 'less_secure', label: 'Less Secure', hint: 'Low-sec' }
	];
	const LIFETIMES = [
		{ value: 'critical', label: 'Critical', hint: '< 1 hour' },
		{ value: 'eol', label: 'End of Life', hint: '< 4 hours' },
		{ value: 'stable', label: 'Healthy Only', hint: '> 4 hours' }
	];
	const MASSES = [
		{ value: 'critical', label: 'Critical Mass', hint: '< 10%' },
		{ value: 'reduced', label: 'Reduced Mass', hint: '< 50%' },
		{ value: 'stable', label: 'High Mass', hint: '> 50%' }
	];

	function update(patch: Record<string, unknown>) {
		map.patchUserSettings(patch).catch(() => {});
	}
</script>

{#snippet options(
	title: string,
	items: { value: string; label: string; hint: string }[],
	current: string,
	key: string
)}
	<div class="flex flex-col gap-1">
		<span class="text-[10px] font-medium tracking-wider text-muted-foreground uppercase">
			{title}
		</span>
		<Select.Root type="single" value={current} onValueChange={(v) => update({ [key]: v })}>
			<Select.Trigger class="h-7 w-full text-xs" data-testid="setting-{key}">
				{items.find((i) => i.value === current)?.label}
			</Select.Trigger>
			<Select.Content>
				<Select.Group>
					{#each items as item (item.value)}
						<Select.Item value={item.value} class="text-xs" label={item.label}>
							<span class="flex w-full items-center justify-between gap-3">
								{item.label}
								<span class="text-xs text-muted-foreground">{item.hint}</span>
							</span>
						</Select.Item>
					{/each}
				</Select.Group>
			</Select.Content>
		</Select.Root>
	</div>
{/snippet}

<Popover.Root>
	<Popover.Trigger
		class="text-muted-foreground transition-colors hover:text-foreground"
		title="Route settings"
		aria-label="Route settings"
		data-testid="route-settings"
	>
		<SlidersHorizontalIcon class="size-3.5" />
	</Popover.Trigger>
	<Popover.Content class="flex w-72 flex-col gap-3 p-3" align="end">
		{@render options('Route Preference', PREFS, preference, 'route_preference')}
		{#if preference !== 'shorter'}
			<div class="flex flex-col gap-1">
				<span class="text-[10px] font-medium tracking-wider text-muted-foreground uppercase">
					Security Penalty: {settings?.security_penalty ?? 50}%
				</span>
				<Slider
					type="single"
					min={0}
					max={100}
					step={5}
					value={settings?.security_penalty ?? 50}
					aria-label="Security penalty"
					onValueCommit={(v) => update({ security_penalty: v })}
				/>
			</div>
		{/if}
		{@render options(
			'Wormhole Lifetime',
			LIFETIMES,
			settings?.route_allow_time_status ?? 'critical',
			'route_allow_time_status'
		)}
		{@render options(
			'Wormhole Mass',
			MASSES,
			settings?.route_allow_mass_status ?? 'reduced',
			'route_allow_mass_status'
		)}
		<label class="flex items-center justify-between gap-2 text-xs">
			Use EVE Scout
			<Switch
				checked={settings?.route_use_evescout ?? false}
				onCheckedChange={(v) => update({ route_use_evescout: v })}
				data-testid="setting-evescout"
			/>
		</label>
	</Popover.Content>
</Popover.Root>
