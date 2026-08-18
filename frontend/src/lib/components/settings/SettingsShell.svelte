<script lang="ts">
	// The frame every settings page sits in: a title, a sidebar of sections, and the page.
	//
	// Sections rather than one long scroll, because settings are looked up rather than read:
	// you arrive knowing which one you want. A single page also gives no way to say what is
	// per-map and what is only yours, which is the distinction people actually get wrong.
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';

	import { page } from '$app/state';
	import { cn } from '$lib/utils';

	export interface Section {
		href: string;
		label: string;
		description: string;
		icon: import('svelte').Component;
	}

	let {
		title,
		subtitle,
		back,
		sections,
		children
	}: {
		title: string;
		subtitle?: string;
		back?: { href: string; label: string };
		sections: Section[];
		children: import('svelte').Snippet;
	} = $props();

	// The longest matching href wins, so `/settings/alerts` does not also light up
	// `/settings`.
	const active = $derived.by(() => {
		const path = page.url.pathname.replace(/\/$/, '');
		return sections
			.map((s) => s.href)
			.filter((href) => path === href || path.startsWith(`${href}/`))
			.sort((a, b) => b.length - a.length)[0];
	});
</script>

<div class="mx-auto flex w-full max-w-5xl flex-col gap-6 py-6">
	{#if back}
		<a
			href={back.href}
			class="flex w-fit items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
		>
			<ArrowLeftIcon class="size-4" />
			{back.label}
		</a>
	{/if}

	<div class="flex flex-col gap-1">
		<h1 class="font-heading text-xl font-semibold tracking-tight">{title}</h1>
		{#if subtitle}
			<p class="text-sm text-muted-foreground">{subtitle}</p>
		{/if}
	</div>

	<div class="flex flex-col gap-6 md:flex-row md:gap-8">
		<nav class="flex shrink-0 gap-1 overflow-x-auto md:w-56 md:flex-col" data-testid="settings-nav">
			{#each sections as section (section.href)}
				{@const Icon = section.icon}
				<a
					href={section.href}
					class={cn(
						'flex items-start gap-2.5 border border-transparent px-3 py-2 text-sm whitespace-nowrap transition-colors md:whitespace-normal',
						section.href === active
							? 'border-border/60 bg-muted/40 text-foreground'
							: 'text-muted-foreground hover:bg-muted/20 hover:text-foreground'
					)}
					data-testid="settings-section"
					data-active={section.href === active}
				>
					<Icon class="mt-0.5 size-4 shrink-0" />
					<span class="flex min-w-0 flex-col">
						<span class="font-medium">{section.label}</span>
						<span class="hidden text-xs text-muted-foreground/80 md:block">
							{section.description}
						</span>
					</span>
				</a>
			{/each}
		</nav>

		<div class="min-w-0 flex-1">
			{@render children()}
		</div>
	</div>
</div>
