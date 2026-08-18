<script lang="ts">
	// A map's settings, in sections.
	//
	// The split is by who a setting belongs to as much as by subject: General, Access and
	// Alerts change the map for everyone on it and are Manager+; Display, Mapping and
	// Routing are yours alone and everyone has them. Sections you cannot use are not shown,
	// rather than shown and refused.
	import BellIcon from '@lucide/svelte/icons/bell';
	import CrosshairIcon from '@lucide/svelte/icons/crosshair';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import RouteIcon from '@lucide/svelte/icons/route';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import UsersIcon from '@lucide/svelte/icons/users';

	import { page } from '$app/state';
	import { api } from '$lib/api/client';
	import type { MapView } from '$lib/api/types/MapView';
	import SettingsShell from '$lib/components/settings/SettingsShell.svelte';
	import type { Section } from '$lib/components/settings/SettingsShell.svelte';

	let { children }: { children: import('svelte').Snippet } = $props();

	const mapId = $derived(Number(page.params.id) || 0);
	let view = $state<MapView | null>(null);

	$effect(() => {
		if (!mapId) return;
		api
			.fetchMap(mapId)
			.then((v) => (view = v))
			.catch(() => {});
	});

	const canManage = $derived(view?.role === 'manager' || view?.role === 'owner');

	const sections = $derived.by<Section[]>(() => {
		const base = `/maps/${mapId}/settings`;
		const mine: Section[] = [
			{
				href: `${base}/display`,
				label: 'Display',
				description: 'What the map shows you',
				icon: EyeIcon
			},
			{
				href: `${base}/mapping`,
				label: 'Mapping',
				description: 'Tracking and scanning',
				icon: CrosshairIcon
			},
			{
				href: `${base}/routing`,
				label: 'Routing',
				description: 'How routes are chosen',
				icon: RouteIcon
			}
		];
		if (!canManage) return mine;
		return [
			{
				href: base,
				label: 'General',
				description: 'Name, naming scheme, deletion',
				icon: SettingsIcon
			},
			...mine,
			{
				href: `${base}/access`,
				label: 'Access',
				description: 'Who can see and edit',
				icon: UsersIcon
			},
			{
				href: `${base}/alerts`,
				label: 'Discord alerts',
				description: 'What gets announced',
				icon: BellIcon
			}
		];
	});
</script>

<SettingsShell
	title={view ? `${view.map.name} settings` : 'Map settings'}
	subtitle="Some of these change the map for everyone on it; the rest are only yours."
	back={{ href: `/maps/${mapId}`, label: 'Back to the map' }}
	{sections}
>
	{@render children()}
</SettingsShell>
