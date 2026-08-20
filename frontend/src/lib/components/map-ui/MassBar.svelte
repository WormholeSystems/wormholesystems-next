<script lang="ts">
	// How much of a wormhole's mass budget is left, with the two thresholds the game moves
	// the hole at: below 50% it reports reduced, below 10% critical. The colour follows the
	// same boundaries, so the bar says the same thing as the status word beside it.
	import * as Tooltip from '$lib/components/ui/tooltip';

	let { remainingPercent }: { remainingPercent: number } = $props();

	const color = $derived(
		remainingPercent <= 10 ? 'bg-red-500' : remainingPercent <= 50 ? 'bg-amber-500' : 'bg-green-500'
	);
</script>

<div class="relative h-1.5 w-full overflow-hidden rounded-full bg-muted">
	<div
		class="h-full rounded-full transition-all {color}"
		style="width: {remainingPercent}%"
		data-testid="mass-bar"
	></div>
	<Tooltip.Root>
		<Tooltip.Trigger class="absolute inset-y-0 left-[10%] flex w-2 -translate-x-1/2 justify-center">
			<span class="h-full w-px bg-popover"></span>
		</Tooltip.Trigger>
		<Tooltip.Content>Below 10% the hole verges to critical</Tooltip.Content>
	</Tooltip.Root>
	<Tooltip.Root>
		<Tooltip.Trigger class="absolute inset-y-0 left-1/2 flex w-2 -translate-x-1/2 justify-center">
			<span class="h-full w-px bg-popover"></span>
		</Tooltip.Trigger>
		<Tooltip.Content>Below 50% the hole shrinks to reduced</Tooltip.Content>
	</Tooltip.Root>
</div>
