<script lang="ts">
	// The spotlight tour over the live map screen. Renders a dimmed overlay with a cutout
	// on the current step's anchor and a card beside it; steps whose anchor is missing
	// (a watcher has no settings link) are skipped.
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';

	import { Button } from '$lib/components/ui/button';
	import { TOUR_STEPS, cardPosition, spotlightRect, type Rect } from './tour';

	let { open = $bindable() }: { open: boolean } = $props();

	const CARD = { w: 320, h: 190 };

	let at = $state(0);
	let spot = $state<Rect | null>(null);
	let resizes = $state(0);
	// The introduction's modal teardown outlives its close by a beat, and it holds a
	// body-wide pointer lock; the tour stays invisible until that lock is gone.
	let ready = $state(false);

	// The steps whose anchors exist right now, measured when the tour opens.
	const present = $derived.by(() => {
		void resizes;
		if (!open) return [];
		return TOUR_STEPS.filter((s) => document.querySelector(s.target) !== null);
	});

	$effect(() => {
		if (!open) {
			ready = false;
			return;
		}
		at = 0;
		// The lock lifts in stages, so one clear reading is not enough: it must stay
		// clear for a few polls before the tour trusts it.
		let clear = 0;
		const settle = setInterval(() => {
			const blocked =
				document.querySelector('[data-slot="dialog-overlay"]') !== null ||
				getComputedStyle(document.body).pointerEvents === 'none';
			clear = blocked ? 0 : clear + 1;
			if (clear >= 3) {
				ready = true;
				resizes += 1;
				clearInterval(settle);
			}
		}, 80);
		const onResize = () => (resizes += 1);
		window.addEventListener('resize', onResize);
		return () => {
			clearInterval(settle);
			window.removeEventListener('resize', onResize);
		};
	});

	$effect(() => {
		void resizes;
		const step = present[at];
		if (!open || !step) return;
		const el = document.querySelector(step.target);
		if (!el) {
			spot = null;
			return;
		}
		const box = el.getBoundingClientRect();
		spot = spotlightRect({ x: box.x, y: box.y, width: box.width, height: box.height }, 8, {
			w: window.innerWidth,
			h: window.innerHeight,
		});
	});

	const card = $derived(
		spot === null
			? null
			: cardPosition(spot, CARD, { w: window.innerWidth, h: window.innerHeight }),
	);

	function next() {
		if (at + 1 >= present.length) {
			open = false;
			return;
		}
		at += 1;
	}
</script>

{#if open && ready && present[at] && spot && card}
	<div class="fixed inset-0 z-70" data-testid="tour" role="dialog" aria-label="Map tour">
		<!-- The shade with a hole in it: four rectangles around the spotlight, so the
		     anchor stays at full brightness and everything else darkens. -->
		{#each [{ x: 0, y: 0, w: '100%', h: spot.y + 'px' }, { x: 0, y: spot.y, w: spot.x + 'px', h: spot.height + 'px' }, { x: spot.x + spot.width, y: spot.y, w: '100%', h: spot.height + 'px' }, { x: 0, y: spot.y + spot.height, w: '100%', h: '100%' }] as shade, i (i)}
			<div
				class="absolute bg-black/55"
				style="left: {shade.x}px; top: {shade.y}px; width: {shade.w}; height: {shade.h};"
			></div>
		{/each}
		<div
			class="absolute rounded-md outline-2 outline-primary/70"
			style="left: {spot.x}px; top: {spot.y}px; width: {spot.width}px; height: {spot.height}px;"
			data-testid="tour-spotlight"
		></div>
		<div
			class="absolute flex w-80 flex-col gap-2 border border-border bg-popover p-4 shadow-lg"
			style="left: {card.x}px; top: {card.y}px;"
			data-testid="tour-card"
		>
			<span class="font-mono text-[10px] tracking-wider text-muted-foreground uppercase">
				Tour · {at + 1} of {present.length}
			</span>
			<span class="font-heading text-base">{present[at].title}</span>
			<p class="text-xs leading-relaxed text-muted-foreground">{present[at].body}</p>
			<div class="mt-1 flex items-center justify-between">
				<button
					class="text-xs text-muted-foreground hover:text-foreground"
					onclick={() => (open = false)}
					data-testid="tour-skip"
				>
					Skip tour
				</button>
				<div class="flex gap-2">
					<Button
						variant="outline"
						size="sm"
						disabled={at === 0}
						onclick={() => (at -= 1)}
						data-testid="tour-back"
					>
						<ArrowLeftIcon data-icon="inline-start" />
						Back
					</Button>
					<Button size="sm" onclick={next} data-testid="tour-next">
						{at + 1 >= present.length ? 'Finish' : 'Next'}
						{#if at + 1 < present.length}
							<ArrowRightIcon data-icon="inline-end" />
						{/if}
					</Button>
				</div>
			</div>
		</div>
	</div>
{/if}
