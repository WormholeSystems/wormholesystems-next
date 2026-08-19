<script lang="ts">
	// What the jump tracker does on your behalf, next to the toggle that turns it on.
	// Stored per user per map, like the route settings popover.
	import SettingsIcon from '@lucide/svelte/icons/settings-2';

	import { Button } from '$lib/components/ui/button';
	import * as Popover from '$lib/components/ui/popover';
	import { Switch } from '$lib/components/ui/switch';
	import type { MapState } from './map-state.svelte';

	let { map }: { map: MapState } = $props();

	const settings = $derived(map.userSettings);
	// Nothing here happens unless the tracker is running, so say so rather than offering
	// switches that quietly do nothing.
	const tracking = $derived(settings?.tracking_allowed ?? false);

	const OPTIONS = [
		{
			key: 'prompt_for_signature' as const,
			label: 'Ask which signature',
			hint: 'Otherwise the hole is mapped without a signature linked.'
		},
		{
			key: 'suggest_alias' as const,
			label: 'Suggest an alias',
			hint: "Prefills the next name in the chain's sequence."
		},
		{
			key: 'copy_bookmark' as const,
			label: 'Copy the bookmark',
			hint: 'Puts the new bookmark on your clipboard once the jump is mapped.'
		}
	];

	function update(key: string, value: boolean) {
		map.patchUserSettings({ [key]: value }).catch(() => {});
	}
</script>

<Popover.Root>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="ghost"
				size="icon"
				class="size-7 text-muted-foreground/50"
				aria-label="Jump tracking settings"
				data-testid="tracking-settings"
			>
				<SettingsIcon />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-72" align="start">
		<div class="flex flex-col gap-3">
			<div class="flex flex-col gap-0.5">
				<span class="text-xs font-medium">Jump tracking</span>
				<span class="text-[11px] text-muted-foreground">
					{tracking
						? 'Flying through an unmapped hole builds it on the map.'
						: 'Turn on Share location to build the map as you fly.'}
				</span>
			</div>
			{#each OPTIONS as option (option.key)}
				<label class="flex items-start justify-between gap-3" for="tracking-{option.key}">
					<span class="flex flex-col gap-0.5">
						<span class="text-xs">{option.label}</span>
						<span class="text-[11px] text-muted-foreground">{option.hint}</span>
					</span>
					<Switch
						id="tracking-{option.key}"
						disabled={!tracking}
						checked={settings?.[option.key] ?? false}
						onCheckedChange={(v) => update(option.key, v)}
						data-testid="setting-{option.key}"
					/>
				</label>
			{/each}
		</div>
	</Popover.Content>
</Popover.Root>
