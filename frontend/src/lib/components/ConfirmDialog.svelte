<script lang="ts">
	// The host behind `confirmDanger`, mounted once in the root layout.
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { confirmState } from '$lib/confirm.svelte';

	const request = $derived(confirmState.pending);
</script>

<AlertDialog.Root
	open={request !== null}
	onOpenChange={(open) => !open && confirmState.settle(false)}
>
	<AlertDialog.Content data-testid="confirm-dialog">
		<AlertDialog.Header>
			<AlertDialog.Title>{request?.title}</AlertDialog.Title>
			{#if request?.body}
				<AlertDialog.Description>{request.body}</AlertDialog.Description>
			{/if}
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel onclick={() => confirmState.settle(false)} data-testid="confirm-cancel">
				{request?.cancel}
			</AlertDialog.Cancel>
			<AlertDialog.Action onclick={() => confirmState.settle(true)} data-testid="confirm-accept">
				{request?.action}
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
