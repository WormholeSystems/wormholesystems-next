<script lang="ts">
	// The signature panel's own rows, driven by static data and given no actions, so what a
	// visitor sees is what the product renders rather than a mock-up of it.
	import type { SignatureCatalog } from '$lib/api/types/SignatureCatalog';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import { loadCatalog } from '$lib/map/signatures';
	import type { SignatureContext } from '$lib/map/signature-context';
	import SignatureRow from './maps/[id]/signatures/SignatureRow.svelte';
	import { DEMO_CONNECTIONS, DEMO_SIGNATURES, DEMO_SYSTEMS, HOME } from './demo-chain';

	// No `actions`, so the rows cannot write anything even if something were clicked.
	const ctx: SignatureContext = {
		systems: DEMO_SYSTEMS,
		connections: DEMO_CONNECTIONS,
		sigs: DEMO_SIGNATURES
	};
	const system = DEMO_SYSTEMS.find((s) => s.id === HOME)!;

	let catalog = $state<SignatureCatalog | null>(null);
	$effect(() => {
		loadCatalog()
			.then((c) => (catalog = c))
			.catch(() => {});
	});
</script>

<div class="overflow-hidden rounded border border-border bg-card">
	<MapPanelHeader>
		Signatures · Turnur
		{#snippet actions()}
			<span class="font-mono text-[10px] text-muted-foreground">{DEMO_SIGNATURES.length}</span>
		{/snippet}
	</MapPanelHeader>
	{#if catalog}
		{#each DEMO_SIGNATURES as sig (sig.id)}
			<SignatureRow
				{ctx}
				{system}
				{sig}
				{catalog}
				compact={false}
				canWrite={false}
				showStaticsFirst={false}
				status={sig.signature_id === 'GHI-789' ? 'new' : null}
			/>
		{/each}
	{:else}
		<!-- Holds the row heights while the catalog arrives, so the band does not jump. -->
		{#each DEMO_SIGNATURES as sig (sig.id)}
			<div class="h-9 border-b border-border/30"></div>
		{/each}
	{/if}
</div>
