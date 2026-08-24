<script lang="ts">
	import { page } from '$app/state';

	import type { MapUserSettings } from '$lib/api/types/MapUserSettings';
	import type { MapView } from '$lib/api/types/MapView';
	import MapScreen from './components/MapScreen.svelte';

	const mapId = $derived(Number(page.params.id) || 0);
	let { data }: { data: { view: MapView | null; settings: MapUserSettings | null } } = $props();
</script>

<!-- Keyed so each map gets its own MapScreen instance: switching maps rebuilds the whole
     object graph, and constructors run at component init where queries are legal. -->
{#key mapId}
	<MapScreen {mapId} seed={data} signedIn={page.data.me != null} />
{/key}
