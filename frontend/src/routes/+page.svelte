<script lang="ts">
	import MapIcon from '@lucide/svelte/icons/map';

	import { Button } from '$lib/components/ui/button';
</script>

<section class="py-20">
	<p class="text-xs font-medium uppercase tracking-[0.25em] text-muted-foreground">EVE Online</p>
	<h1 class="mt-3 font-heading text-4xl font-semibold tracking-tight">Vector</h1>
	<p class="mt-3 max-w-prose text-muted-foreground">
		Real-time wormhole mapping — chains, signatures, and live pilot tracking.
	</p>
	<Button href="/maps" size="lg" class="mt-8">
		<MapIcon data-icon="inline-start" />
		Open your maps
	</Button>
</section>
