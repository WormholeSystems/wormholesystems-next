<script lang="ts">
	// Signature panel for the single selected system: paste-to-import + a list with
	// link/unlink and remove.
	import Link2Icon from '@lucide/svelte/icons/link-2';
	import XIcon from '@lucide/svelte/icons/x';

	import { api } from '$lib/api/client';
	import type { MapSystemView } from '$lib/api/types/MapSystemView';
	import { Button } from '$lib/components/ui/button';
	import MapPanel from '$lib/components/map-panel/MapPanel.svelte';
	import MapPanelContent from '$lib/components/map-panel/MapPanelContent.svelte';
	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { groupColor, parseScan } from '$lib/map/helpers';
	import type { MapState } from './map-state.svelte';

	let { map, system }: { map: MapState; system: MapSystemView } = $props();

	let pasteText = $state('');

	// Connections with an endpoint in this system → (connection id, other-system label).
	const conns = $derived.by(() => {
		const label = (pid: number) => {
			const s = map.systems.find((s) => s.id === pid);
			return s ? (s.alias ?? s.name) : '?';
		};
		return map.connections
			.filter((c) => c.from_system === system.id || c.to_system === system.id)
			.map((c) => ({
				id: c.id,
				label: `→ ${label(c.from_system === system.id ? c.to_system : c.from_system)}`
			}));
	});

	const mySigs = $derived(
		map.sigs
			.filter((s) => s.solar_system_id === system.solar_system_id)
			.toSorted((a, b) => a.signature_id.localeCompare(b.signature_id))
	);

	function applyPaste() {
		const parsed = parseScan(pasteText);
		if (parsed.length === 0) {
			map.statusLine = 'no signatures parsed';
			return;
		}
		const cmd = {
			map_id: map.mapId,
			solar_system_id: system.solar_system_id,
			signatures: parsed
		};
		pasteText = '';
		map.run('paste sigs', api.pasteSignatures(cmd));
	}

	function remove(pk: number) {
		map.run('rm sig', api.removeSignature({ map_id: map.mapId, signature_pk: pk }));
	}

	function unlink(pk: number) {
		map.run('unlink', api.unlinkSignature({ map_id: map.mapId, signature_pk: pk }));
	}

	function link(pk: number, value: string) {
		const connectionId = Number(value);
		if (!Number.isFinite(connectionId) || connectionId === 0) return;
		map.run(
			'link',
			api.linkSignature({ map_id: map.mapId, signature_pk: pk, connection_id: connectionId })
		);
	}
</script>

<MapPanel testid="signatures-card">
	<MapPanelHeader>
		Signatures
		{#if mySigs.length > 0}
			<span class="ml-1 text-amber-400">{mySigs.length}</span>
		{/if}
	</MapPanelHeader>
	<MapPanelContent>
		<div class="flex flex-col gap-2 p-3 text-xs">
	<Textarea
		class="h-16 min-h-16 resize-none font-mono text-[10px]"
		placeholder="Paste in-game scan results…"
		bind:value={pasteText}
	/>
	<Button variant="secondary" onclick={applyPaste}>Apply paste</Button>
	<ul class="flex max-h-48 flex-col gap-0.5 overflow-auto">
		{#each mySigs as s (s.id)}
			{@const isWh = s.group === 'wormhole'}
			<li class="flex items-center gap-1">
				<span class="font-mono text-muted-foreground">{s.signature_id}</span>
				<span class="text-[9px] uppercase" style:color={groupColor(s.group)}>{s.group}</span>
				<span class="truncate text-foreground">{s.name ?? ''}</span>
				{#if isWh && s.connection_id !== null}
					<Button
						variant="ghost"
						size="icon-xs"
						class="text-emerald-400 hover:text-emerald-300"
						title="unlink"
						onclick={() => unlink(s.id)}
					>
						<Link2Icon />
					</Button>
				{:else if isWh}
					<Select.Root type="single" onValueChange={(v) => link(s.id, v)}>
						<Select.Trigger size="sm" class="ml-auto h-5 max-w-24 px-1 text-[9px]">
							link…
						</Select.Trigger>
						<Select.Content>
							<Select.Group>
								{#each conns as c (c.id)}
									<Select.Item value={String(c.id)}>{c.label}</Select.Item>
								{/each}
							</Select.Group>
						</Select.Content>
					</Select.Root>
				{/if}
				<Button
					variant="ghost"
					size="icon-xs"
					class="ml-auto text-muted-foreground hover:text-destructive"
					onclick={() => remove(s.id)}
				>
					<XIcon />
				</Button>
			</li>
		{/each}
		</ul>
		</div>
	</MapPanelContent>
</MapPanel>
