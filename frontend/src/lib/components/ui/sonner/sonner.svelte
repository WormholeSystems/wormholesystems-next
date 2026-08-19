<script lang="ts">
	// The app owns its own dark mode (a class on <html>, persisted), so the toasts follow
	// that rather than mode-watcher, which nothing else here uses.
	import { Toaster as Sonner, type ToasterProps as SonnerProps } from "svelte-sonner";
	import { theme } from '$lib/theme.svelte';
	import Loader2Icon from '@lucide/svelte/icons/loader-2';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import OctagonXIcon from '@lucide/svelte/icons/octagon-x';
	import InfoIcon from '@lucide/svelte/icons/info';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';

	let { ...restProps }: SonnerProps = $props();
</script>

<!--
	Squared off, thin-bordered and small, like the map's own panels and status bar: a toast
	is one more readout on the same instrument, not a notification from somewhere else.
	The kind is carried by the icon's colour rather than by a filled background, which at
	this size reads as an alarm. The close button is pulled inside the toast's own right
	edge; sonner's default hangs it half outside the top-left corner, where it reads as a
	box of its own rather than as part of the toast.
-->
<Sonner
	theme={theme.dark ? 'dark' : 'light'}
	class="toaster group"
	gap={6}
	style="--normal-bg: var(--color-card); --normal-text: var(--color-foreground); --normal-border: var(--color-border); --toast-close-button-start: auto; --toast-close-button-end: 6px; --toast-close-button-transform: translateY(-50%);"
	toastOptions={{
		classes: {
			toast:
				'!rounded-none !border !border-border !bg-card !text-foreground !shadow-none !ring-1 !ring-foreground/10 !gap-2 !px-3 !py-2',
			title: '!text-xs !font-medium',
			description: '!text-[11px] !text-muted-foreground !font-mono',
			icon: '!mr-0',
			closeButton:
				'!top-1/2 !rounded-none !border-0 !bg-transparent !text-muted-foreground hover:!text-foreground',
			success: '[&_[data-icon]]:text-emerald-500',
			error: '[&_[data-icon]]:text-destructive',
			info: '[&_[data-icon]]:text-sky-500',
			warning: '[&_[data-icon]]:text-amber-500'
		}
	}}
	{...restProps}
>
	{#snippet loadingIcon()}
		<Loader2Icon class="size-4 animate-spin" />
	{/snippet}
	{#snippet successIcon()}
		<CircleCheckIcon class="size-4" />
	{/snippet}
	{#snippet errorIcon()}
		<OctagonXIcon class="size-4" />
	{/snippet}
	{#snippet infoIcon()}
		<InfoIcon class="size-4" />
	{/snippet}
	{#snippet warningIcon()}
		<TriangleAlertIcon class="size-4" />
	{/snippet}
</Sonner>
