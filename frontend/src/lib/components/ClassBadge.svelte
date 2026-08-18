<script lang="ts">
	// A solar system's class letter (C5, H, L, NS, P…) in its class colour.
	//
	// The colour is applied as a CSS variable rather than a utility class on purpose: the
	// class tokens live in a plain `:root` block, not in `@theme`, so Tailwind never
	// generates `text-c5` and a class-based version silently renders in whatever colour it
	// inherits. Every call site went through this component so that cannot happen again.
	import { classMeta } from '$lib/map/classes';
	import { cn } from '$lib/utils';

	let {
		classId,
		security,
		class: className,
		title
	}: {
		classId: number | null;
		/** Security status, used for the k-space classes that have no class id. */
		security: number | null;
		class?: string;
		title?: string;
	} = $props();

	const meta = $derived(classMeta(classId, security));
</script>

<span
	class={cn('font-mono', className)}
	style="color: var(--color-{meta.token})"
	{title}
	data-testid="class-badge"
	data-class={meta.short}
>
	{meta.short}
</span>
