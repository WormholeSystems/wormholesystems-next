<script lang="ts">
	// "Which signature did you jump?", the prompt that turns a jump into a mapped hole.
	// It opens mid-flight, so it has to be answerable without the mouse: the search field keeps
	// focus, the arrow keys walk the list, and the likeliest signature starts selected.
	import * as Dialog from '$lib/components/ui/dialog';
	import type { MapState } from '../state/map-state.svelte';
	import type { JumpTracker } from '../state/tracking.svelte';
	import TrackingForm from './TrackingForm.svelte';

	let { map, tracker }: { map: MapState; tracker: JumpTracker } = $props();

	const prompt = $derived(tracker.prompt);
	const open = $derived(prompt !== null);
</script>

<Dialog.Root
	{open}
	onOpenChange={(next) => {
		// Dismissing is a deliberate "not now": the hole stays unmapped rather than guessed at.
		if (!next) tracker.dismiss();
	}}
>
	<Dialog.Content class="max-w-lg gap-0 overflow-hidden p-0" data-testid="tracking-dialog">
		{#if prompt}
			<!-- Keyed on the prompt itself, so a jump arriving while the dialog is open starts
			     the form clean instead of keeping the previous answers. -->
			{#key prompt}
				<TrackingForm {map} {tracker} {prompt} />
			{/key}
		{/if}
	</Dialog.Content>
</Dialog.Root>
