<script lang="ts">
	// One side of the hole: the resolved type, scanner id, and destination class.
	import type { Signature } from '$lib/api/types/Signature';
	import type { SignatureCatalog } from '$lib/api/types/SignatureCatalog';
	import { destClassMeta } from '$lib/map/classes';
	import { typeById } from '$lib/map/signatures';

	let { title, sig, catalog }: { title: string; sig: Signature; catalog: SignatureCatalog } =
		$props();

	const type = $derived(typeById(catalog, sig.signature_type_id));
	const dest = $derived(type?.target_class == null ? null : destClassMeta(type.target_class));
</script>

<div class="space-y-1">
	<div class="border-b pb-1 text-xs font-medium text-foreground">{title}</div>
	<div class="grid grid-cols-2 divide-y truncate text-xs text-muted-foreground *:py-1">
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Type</span>
			<span class="truncate text-right">{type?.name ?? sig.name ?? 'Unknown'}</span>
		</div>
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Signature ID</span>
			<span class="text-right">{sig.signature_id || 'Unknown'}</span>
		</div>
		{#if dest}
			<div class="col-span-full grid grid-cols-subgrid">
				<span>Leads To</span>
				<span class="flex justify-end text-right" style="color: var(--color-{dest.token})">
					{dest.short}
				</span>
			</div>
		{/if}
	</div>
</div>
