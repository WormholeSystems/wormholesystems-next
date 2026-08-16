<script lang="ts">
	// The Status section (legacy ConnectionStatus): type / lifetime (with "EOL since"
	// tooltip) / mass status / preserve-mass / created / updated. Created is the
	// earliest and Updated the latest across the connection and its linked signatures.
	import HeartIcon from '@lucide/svelte/icons/heart';

	import type { MapConnection } from '$lib/api/types/MapConnection';
	import type { Signature } from '$lib/api/types/Signature';
	import * as Tooltip from '$lib/components/ui/tooltip';

	let { connection, sigs }: { connection: MapConnection; sigs: Signature[] } = $props();

	let now = $state(Date.now());
	$effect(() => {
		const t = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(t);
	});

	const typeMeta = $derived(
		connection.kind === 'stargate'
			? { label: 'Stargate', text: 'text-sky-500', dot: 'bg-sky-500' }
			: { label: 'Wormhole', text: 'text-foreground', dot: 'bg-neutral-500' }
	);
	const lifetimeMeta = $derived.by(() => {
		switch (connection.time_status) {
			case 'eol':
				return { label: 'End of Life (<4h)', text: 'text-purple-500', dot: 'bg-purple-500' };
			case 'critical':
				return { label: 'Critical (<1h)', text: 'text-red-500', dot: 'bg-red-500' };
			case 'stable':
				return { label: 'Healthy', text: 'text-green-500', dot: 'bg-green-500' };
			default:
				return { label: 'Healthy', text: 'text-green-500', dot: 'bg-green-500' };
		}
	});
	const massMeta = $derived.by(() => {
		switch (connection.mass_status) {
			case 'stable':
				return { label: 'Fresh', text: 'text-green-500', dot: 'bg-green-500' };
			case 'reduced':
				return { label: 'Reduced', text: 'text-amber-500', dot: 'bg-amber-500' };
			case 'critical':
				return { label: 'Critical', text: 'text-red-500', dot: 'bg-red-500' };
			default:
				return { label: 'Unknown', text: 'text-muted-foreground', dot: 'bg-neutral-500' };
		}
	});

	const createdMs = $derived(
		Math.min(Date.parse(connection.created_at), ...sigs.map((s) => Date.parse(s.created_at)))
	);
	const updatedMs = $derived(
		Math.max(Date.parse(connection.updated_at), ...sigs.map((s) => Date.parse(s.updated_at)))
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

	function ago(ms: number): string {
		const mins = Math.floor((now - ms) / 60_000);
		if (mins < 1) return 'just now';
		if (mins < 60) return `${mins}m ago`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours}h ago`;
		return `${Math.floor(hours / 24)}d ago`;
	}

	const degraded = $derived(
		connection.time_status === 'eol' || connection.time_status === 'critical'
	);
	const lifetimeSince = $derived(
		connection.time_status_updated_at === null
			? null
			: Date.parse(connection.time_status_updated_at)
	);
</script>

<div class="space-y-1">
	<div class="border-b pb-1 text-xs font-medium text-foreground">Status</div>
	<div class="grid grid-cols-2 divide-y truncate text-xs text-muted-foreground *:py-1">
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Type</span>
			<span class="flex items-center justify-end gap-1.5 text-right {typeMeta.text}">
				<span class="inline-block size-2 rounded-full {typeMeta.dot}"></span>
				{typeMeta.label}
			</span>
		</div>
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Lifetime</span>
			<span
				class="flex items-center justify-end gap-1.5 text-right {lifetimeMeta.text}"
				data-testid="popover-lifetime"
			>
				<span class="inline-block size-2 shrink-0 rounded-full {lifetimeMeta.dot}"></span>
				{#if degraded && lifetimeSince !== null}
					<Tooltip.Root>
						<Tooltip.Trigger class="cursor-help">{lifetimeMeta.label}</Tooltip.Trigger>
						<Tooltip.Content>{fmt(lifetimeSince)} ({ago(lifetimeSince)})</Tooltip.Content>
					</Tooltip.Root>
				{:else}
					<span>{lifetimeMeta.label}</span>
				{/if}
			</span>
		</div>
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Mass Status</span>
			<span class="flex items-center justify-end gap-1.5 text-right {massMeta.text}">
				<span class="inline-block size-2 rounded-full {massMeta.dot}"></span>
				{massMeta.label}
			</span>
		</div>
		{#if connection.preserve_mass}
			<div class="col-span-full grid grid-cols-subgrid" data-testid="popover-preserve-mass">
				<span>Preserve mass</span>
				<span class="flex items-center justify-end gap-1.5 text-right text-emerald-500">
					<HeartIcon class="size-3" />
					Yes
				</span>
			</div>
		{/if}
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Created</span>
			<Tooltip.Root>
				<Tooltip.Trigger class="cursor-help text-right">{ago(createdMs)}</Tooltip.Trigger>
				<Tooltip.Content>{fmt(createdMs)}</Tooltip.Content>
			</Tooltip.Root>
		</div>
		<div class="col-span-full grid grid-cols-subgrid">
			<span>Updated</span>
			<Tooltip.Root>
				<Tooltip.Trigger class="cursor-help text-right">{ago(updatedMs)}</Tooltip.Trigger>
				<Tooltip.Content>{fmt(updatedMs)}</Tooltip.Content>
			</Tooltip.Root>
		</div>
	</div>
</div>
