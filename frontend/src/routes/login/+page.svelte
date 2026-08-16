<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
</script>

<div class="grid min-h-[60vh] place-items-center">
	<Card.Root class="w-full max-w-sm text-center">
		<Card.Header>
			<Card.Title>Sign in</Card.Title>
			<Card.Description>Authenticate with your EVE Online character.</Card.Description>
		</Card.Header>
		<Card.Content>
			<Button href="/auth/login" data-sveltekit-reload class="w-full">
				Log in with EVE Online
			</Button>
		</Card.Content>
	</Card.Root>
</div>
