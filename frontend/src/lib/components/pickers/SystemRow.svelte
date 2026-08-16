<script lang="ts">
	// One solar system result row, shared by every system picker: colored class badge,
	// name, region.
	import type { SystemSearchResult } from '$lib/api/types/SystemSearchResult';
	import { searchClassification } from '$lib/map/classes';

	let { system }: { system: SystemSearchResult } = $props();

	const c = $derived(searchClassification(system));
</script>

<span class="w-12 shrink-0 font-mono text-xs" style="color: var(--color-{c.token})">{c.badge}</span>
<span class="truncate text-foreground">{system.name}</span>
<span class="ml-auto truncate text-xs text-muted-foreground">{system.region}</span>
