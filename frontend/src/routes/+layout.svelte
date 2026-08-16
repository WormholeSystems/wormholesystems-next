<script lang="ts">
	import '../app.css';

	import Nav from '$lib/components/Nav.svelte';

	let { children, data } = $props();

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
<main class="p-6">
	{@render children()}
</main>
