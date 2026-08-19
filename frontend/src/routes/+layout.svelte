<script lang="ts">
	import '../app.css';

	import { page } from '$app/state';

	import Nav from '$lib/components/Nav.svelte';
	import { Toaster } from '$lib/components/ui/sonner';

	let { children, data } = $props();

	// The map page fills the window edge to edge; every other page wants the usual padding.
	const flush = $derived(/^\/maps\/\d+$/.test(page.url.pathname));

	// Marks the page as interactive; e2e tests wait for this before clicking, since
	// SSR-rendered controls are dead until hydration attaches their handlers.
	$effect(() => {
		document.documentElement.dataset.hydrated = 'true';
	});
</script>

<svelte:head>
	<title>Vector</title>
</svelte:head>

<Toaster position="top-center" closeButton />
<Nav me={data.me} maps={data.maps} status={data.status} />
<main class={flush ? "" : "p-6"}>
	{@render children()}
</main>
