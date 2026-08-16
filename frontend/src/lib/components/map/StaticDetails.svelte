<script lang="ts">
	// Wormhole physics for one static, the tooltip/popover body (legacy StaticDetails).
	import type { Static } from '$lib/api/types/Static';
	import { destClassMeta } from '$lib/map/classes';

	let { static: st }: { static: Static } = $props();

	const dest = $derived(destClassMeta(st.dest_class));

	function kt(kg: number | null): string {
		if (kg === null) return '?';
		return `${(kg / 1_000_000).toLocaleString('en-US')} kt`;
	}

	// Legacy ship-size classes by max jump mass (millions of kg).
	function shipSize(kg: number | null): string {
		if (kg === null) return '?';
		const m = kg / 1_000_000;
		if (m >= 1000) return 'XL';
		if (m >= 62) return 'L';
		if (m >= 5) return 'M';
		return 'S';
	}
</script>

<div class="w-44 text-[11px]">
	<div class="flex items-center justify-between border-b border-border px-2 py-1 font-medium">
		{st.code}
		<span style:color="var(--color-{dest.token})">{dest.short}</span>
	</div>
	<div class="flex flex-col gap-0.5 px-2 py-1">
		<div class="flex justify-between gap-2">
			<span class="text-muted-foreground">Total Mass</span>
			<span>{kt(st.total_mass)}</span>
		</div>
		<div class="flex justify-between gap-2">
			<span class="text-muted-foreground">Max Jump Mass</span>
			<span>{kt(st.max_jump_mass)}</span>
		</div>
		<div class="flex justify-between gap-2">
			<span class="text-muted-foreground">Ship Size</span>
			<span>{shipSize(st.max_jump_mass)}</span>
		</div>
		<div class="flex justify-between gap-2">
			<span class="text-muted-foreground">Lifetime</span>
			<span>{st.lifetime_hours === null ? '?' : `${st.lifetime_hours} h`}</span>
		</div>
		<div class="flex justify-between gap-2">
			<span class="text-muted-foreground">Sig Strength</span>
			<span>{st.signature_strength === null ? '?' : `${st.signature_strength}%`}</span>
		</div>
	</div>
</div>
