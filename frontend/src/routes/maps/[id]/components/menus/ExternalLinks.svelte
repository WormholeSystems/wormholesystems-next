<script lang="ts">
	// The External submenu in the on-map hover chrome; SystemMenu renders the same
	// groups through bits-ui.
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import ExternalLinkIcon from '@lucide/svelte/icons/external-link';

	import type { SystemLinkGroup } from '$lib/map/external-links';
	import { item, panel, sub } from './chrome';

	let { groups }: { groups: SystemLinkGroup[] } = $props();
</script>

<div class={sub} data-testid="external-subtrigger">
	<ExternalLinkIcon class="size-4" />
	External
	<ChevronRightIcon class="ml-auto size-3" />
	<div class={panel} data-testid="external-submenu">
		{#each groups as group, i (group.label)}
			{#if i > 0}
				<div class="my-0.5 border-t border-border"></div>
			{/if}
			<div
				class="px-3 py-1 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase"
			>
				{group.label}
			</div>
			{#each group.links as link (link.label)}
				<a class={item} href={link.href} target="_blank" rel="noopener">
					<link.icon class="size-4" />
					{link.label}
				</a>
			{/each}
		{/each}
	</div>
</div>
