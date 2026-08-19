<script lang="ts">
	// Relative age, with the linked connection's state bleeding into the colouring: the
	// connection's mass wins, and its lifetime applies while the signature's own is healthy.
	import type { MapConnection } from '$lib/api/types/MapConnection';
	import type { Signature } from '$lib/api/types/Signature';
	import * as Tooltip from '$lib/components/ui/tooltip';

	let {
		sig,
		connection,
		compact
	}: { sig: Signature; connection: MapConnection | null; compact: boolean } = $props();

	let now = $state(Date.now());
	$effect(() => {
		const t = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(t);
	});

	// Wormhole ages run from creation (earliest of signature or connection); sites from the
	// last update.
	const baseDate = $derived.by(() => {
		if (sig.group === 'wormhole') {
			const times = [Date.parse(sig.created_at)];
			if (connection) times.push(Date.parse(connection.created_at));
			return Math.min(...times);
		}
		return Date.parse(sig.updated_at);
	});

	const ago = $derived.by(() => {
		const mins = Math.floor((now - baseDate) / 60_000);
		if (mins < 1) return 'now';
		if (mins < 60) return `${mins}m`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours}h`;
		return `${Math.floor(hours / 24)}d`;
	});

	const lifetime = $derived(
		sig.time_status && sig.time_status !== 'stable'
			? sig.time_status
			: (connection?.time_status ?? sig.time_status)
	);
	const mass = $derived(connection?.mass_status ?? sig.mass_status);
	const lifetimeSince = $derived(
		sig.time_status_updated_at ?? connection?.time_status_updated_at ?? null
	);

	function fmt(ms: number): string {
		return new Date(ms).toLocaleString('en-US', {
			month: 'short',
			day: '2-digit',
			hour: '2-digit',
			minute: '2-digit',
			hour12: false,
			timeZone: 'UTC'
		});
	}

	function sinceAgo(iso: string): string {
		const mins = Math.max(0, Math.floor((now - Date.parse(iso)) / 60_000));
		if (mins < 60) return `${mins} minutes ago`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours} hours ago`;
		return `${Math.floor(hours / 24)} days ago`;
	}
</script>

<Tooltip.Root>
	<Tooltip.Trigger
		data-lifetime={lifetime && lifetime !== 'stable' ? lifetime : null}
		data-mass={mass && mass !== 'stable' ? mass : null}
		class="sig-time flex w-full items-center justify-end font-mono text-xs whitespace-nowrap text-muted-foreground tabular-nums data-[lifetime=critical]:text-red-500 data-[lifetime=eol]:text-purple-500 data-[mass=critical]:text-red-500 data-[mass=reduced]:text-orange-500 {compact
			? 'h-5'
			: 'h-6'}"
	>
		<span>{ago}</span>
	</Tooltip.Trigger>
	<Tooltip.Content>
		<div class="grid grid-cols-[auto_auto] gap-x-2 gap-y-0.5 text-xs">
			<span class="font-semibold">Created at</span>
			<span>{fmt(baseDate)}</span>
			<span class="font-semibold">Last modified at</span>
			<span>{fmt(Date.parse(sig.updated_at))}</span>
			{#if lifetime === 'eol' || lifetime === 'critical'}
				<span class="font-semibold">
					{lifetime === 'eol' ? 'End of Life (<4h)' : 'Critical (<1h)'}
				</span>
				<span>{lifetimeSince ? sinceAgo(lifetimeSince) : ''}</span>
			{/if}
		</div>
	</Tooltip.Content>
</Tooltip.Root>
