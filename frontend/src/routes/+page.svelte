<script lang="ts">
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import GitForkIcon from '@lucide/svelte/icons/git-fork';
	import MapIcon from '@lucide/svelte/icons/map';

	import { Button } from '$lib/components/ui/button';
	import { gridBackground } from '$lib/map/helpers';
	import LandingChain from './LandingChain.svelte';

	// One tile each, in the order someone evaluating this would ask about them. Two of them
	// take a double column, which is what fills the four-wide grid exactly.
	const features = [
		{
			wide: true,
			label: 'Signatures',
			title: 'Paste the scanner, keep the chain',
			body: 'Drop probe-scanner output in and the panel diffs it: new signatures appear, renamed ones keep their link, and a wormhole you have flown ties itself to the connection it opened.'
		},
		{
			wide: false,
			label: 'Routing',
			title: 'The shortest way home',
			body: 'Routes run through the chain and the stargate graph together, weighted by security, and by how much lifetime and mass you are willing to trust.'
		},
		{
			wide: false,
			label: 'Pilots',
			title: 'Who is where, live',
			body: 'Everyone who opts in shares their position from ESI. The map shows where they are, how far that is from you, and what they are flying.'
		},
		{
			wide: true,
			label: 'Connections',
			title: 'Mass and lifetime that add up',
			body: 'Every jump is recorded against the hole it went through, so a connection knows what it has taken. Critical ones surface before someone rolls into them.'
		},
		{
			wide: false,
			label: 'Alerts',
			title: 'Told in Discord',
			body: 'Watch a system, a range of jumps, or a chain, and get told there. Slash commands answer from the same map, and accounts link to their pilots.'
		},
		{
			wide: false,
			label: 'Access',
			title: 'Who sees what',
			body: 'Viewer, member, manager, owner. Share a link for people outside the map entirely, and withdraw it without touching anyone else.'
		}
	];

	const install = [
		{ cmd: 'git clone git@github.com:eve-vector/vector.git', note: null },
		{ cmd: './vectorctl setup', note: 'asks for what it needs, checks the rest' }
	];

	const checks = [
		'Docker and Compose v2',
		'the domain resolves here',
		'ports 80 and 443 are free',
		'disk for the static data',
		'the checkout is current'
	];
</script>

<svelte:head>
	<title>Vector — wormhole mapping for EVE Online</title>
	<meta
		name="description"
		content="Real-time collaborative wormhole mapping for EVE Online. Open source and self-hosted."
	/>
</svelte:head>

<!-- Hero: the product's own canvas, grid and all. -->
<section class="relative overflow-hidden border-b border-border">
	<div
		class="pointer-events-none absolute inset-0 opacity-70"
		style:background-image={gridBackground()}
		style:background-size="40px 40px"
	></div>
	<div
		class="relative mx-auto flex w-full max-w-7xl flex-col items-center gap-10 px-6 py-20 lg:flex-row lg:justify-between lg:py-28"
	>
		<div class="max-w-xl">
			<p class="font-mono text-[11px] tracking-[0.25em] text-muted-foreground uppercase">
				EVE Online · Wormhole mapping
			</p>
			<h1 class="mt-4 font-heading text-5xl font-semibold tracking-tight sm:text-6xl">
				Map the chain<br />together.
			</h1>
			<p class="mt-5 max-w-prose text-muted-foreground">
				One live map for the whole corp. Signatures, mass, lifetime and everyone's position, in
				step for everybody looking at it. Run it on your own machine.
			</p>
			<div class="mt-8 flex flex-wrap items-center gap-3">
				<Button href="/maps" size="lg">
					<MapIcon data-icon="inline-start" />
					Open your maps
				</Button>
				<Button href="#self-host" size="lg" variant="outline">
					Self-host it
					<ArrowRightIcon data-icon="inline-end" />
				</Button>
			</div>
		</div>
		<LandingChain />
	</div>
</section>

<!-- Features, dense and asymmetric: two double-width tiles fill the four-wide grid exactly. -->
<section class="mx-auto w-full max-w-7xl px-6 py-16">
	<div class="grid gap-px border border-border bg-border sm:grid-cols-2 lg:grid-cols-4">
		{#each features as feature (feature.label)}
			<article
				class="flex flex-col gap-2 bg-background p-6 {feature.wide ? 'sm:col-span-2' : ''}"
				data-testid="landing-feature"
			>
				<p class="font-mono text-[10px] tracking-[0.2em] text-muted-foreground uppercase">
					{feature.label}
				</p>
				<h2 class="font-heading text-lg font-medium tracking-tight">{feature.title}</h2>
				<p class="text-sm text-muted-foreground">{feature.body}</p>
			</article>
		{/each}
	</div>
</section>

<!-- Self-hosting: the actual commands, and what setup does before it asks for anything. -->
<section id="self-host" class="border-y border-border bg-card/40">
	<div class="mx-auto grid w-full max-w-7xl gap-px bg-border lg:grid-cols-3">
		<div class="bg-background p-6 lg:col-span-2">
			<p class="font-mono text-[10px] tracking-[0.2em] text-muted-foreground uppercase">
				Self-hosted
			</p>
			<h2 class="mt-2 font-heading text-lg font-medium tracking-tight">
				Two commands on one machine
			</h2>
			<div class="mt-4 border border-border bg-card p-4 font-mono text-xs">
				{#each install as line (line.cmd)}
					<p class="flex items-baseline gap-2 py-0.5">
						<span class="text-muted-foreground select-none">$</span>
						<!-- The command wraps under the prompt rather than pushing it onto its own line. -->
						<span class="min-w-0 break-all">
							{line.cmd}
							{#if line.note}
								<span class="text-muted-foreground">&nbsp;# {line.note}</span>
							{/if}
						</span>
					</p>
				{/each}
			</div>
			<p class="mt-4 text-sm text-muted-foreground">
				Postgres, the API, the web server and TLS come up together under Docker. Certificates are
				issued on the first boot and the static data seeds itself.
			</p>
		</div>
		<div class="bg-background p-6">
			<p class="font-mono text-[10px] tracking-[0.2em] text-muted-foreground uppercase">
				Checked first
			</p>
			<h2 class="mt-2 font-heading text-lg font-medium tracking-tight">
				Before it asks for anything
			</h2>
			<ul class="mt-4 flex flex-col gap-2 text-sm text-muted-foreground">
				{#each checks as check (check)}
					<li class="flex items-baseline gap-2">
						<span class="font-mono text-[10px] text-muted-foreground/60">✓</span>
						{check}
					</li>
				{/each}
			</ul>
		</div>
	</div>
</section>

<section class="mx-auto w-full max-w-7xl px-6 py-16">
	<div class="flex flex-col items-start justify-between gap-6 sm:flex-row sm:items-end">
		<div class="max-w-xl">
			<p class="font-mono text-[10px] tracking-[0.2em] text-muted-foreground uppercase">
				Open source
			</p>
			<h2 class="mt-2 font-heading text-2xl font-medium tracking-tight">
				Your map, your database, your machine
			</h2>
			<p class="mt-3 text-sm text-muted-foreground">
				Nothing about your chain leaves the box you put it on. Read the code, run it, change it.
			</p>
		</div>
		<Button href="https://github.com/eve-vector/vector" variant="outline" size="lg">
			<GitForkIcon data-icon="inline-start" />
			Source on GitHub
		</Button>
	</div>
</section>
