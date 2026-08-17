<script lang="ts">
	import '../app.css';

	import { page } from '$app/state';

	import Nav from '$lib/components/Nav.svelte';

	let { children, data } = $props();

	// The map fills the window edge to edge and sits flush under the nav; every other page
	// wants the usual page padding.
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

<Nav me={data.me} />
<main class={flush ? "" : "p-6"}>
	{@render children()}
</main>
