<script lang="ts">
	// Warn before pasting into a system the active character is not in.
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';

	let {
		open = $bindable(false),
		targetLabel,
		characterSystem,
		onconfirm,
		oncancel,
	}: {
		open: boolean;
		targetLabel: string;
		characterSystem: string;
		onconfirm: () => void;
		oncancel: () => void;
	} = $props();

	function close(confirmed: boolean) {
		open = false;
		if (confirmed) onconfirm();
		else oncancel();
	}
</script>

<Dialog.Root
	bind:open
	onOpenChange={(o) => {
		if (!o) oncancel();
	}}
>
	<Dialog.Content class="max-w-md" data-testid="paste-mismatch">
		<Dialog.Header>
			<Dialog.Title>System Mismatch Warning</Dialog.Title>
			<Dialog.Description>
				You are pasting signatures into
				<strong class="text-foreground">{targetLabel}</strong>, but your tracked character is
				currently in <strong class="text-foreground">{characterSystem}</strong>.
			</Dialog.Description>
		</Dialog.Header>
		<div class="rounded-lg border border-yellow-500/20 bg-yellow-500/10 p-4">
			<p class="text-sm text-foreground">
				Are you sure you want to paste signatures into a different system than where your character
				is located?
			</p>
		</div>
		<Dialog.Footer class="gap-2">
			<Button variant="outline" onclick={() => close(false)}>Cancel</Button>
			<Button onclick={() => close(true)}>Paste Anyway</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
