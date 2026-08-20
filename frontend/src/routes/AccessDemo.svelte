<script lang="ts">
	// The access list as the settings page draws it: who holds a grant, what kind of thing
	// they are, and what that lets them do. The role names and the sentences under them are
	// the ones the app itself shows, not a second copy written for this page.
	import BuildingIcon from '@lucide/svelte/icons/building-2';
	import UserIcon from '@lucide/svelte/icons/user';
	import UsersIcon from '@lucide/svelte/icons/users';

	import MapPanelHeader from '$lib/components/map-panel/MapPanelHeader.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { ROLE_HELP, ROLE_LABEL, ROLES_ASCENDING } from '$lib/map/roles';
	import type { Role } from '$lib/api/types/Role';

	const KIND_ICON = { character: UserIcon, corporation: BuildingIcon, alliance: UsersIcon };

	const grants: { name: string; kind: keyof typeof KIND_ICON; role: Role; expires?: string }[] = [
		{ name: 'Nicolas Kion', kind: 'character', role: 'owner' },
		{ name: 'Hole Control', kind: 'alliance', role: 'manager' },
		{ name: 'Wandering Phoenix', kind: 'corporation', role: 'member' },
		{ name: 'Tovan Khev', kind: 'character', role: 'viewer', expires: 'in 7 days' }
	];
</script>

<div class="grid gap-6 lg:grid-cols-[1fr_1fr] lg:gap-8">
	<div class="overflow-hidden rounded border border-border bg-card">
		<MapPanelHeader>
			Access · home.map
			{#snippet actions()}
				<span class="font-mono text-[10px] text-muted-foreground">{grants.length}</span>
			{/snippet}
		</MapPanelHeader>
		{#each grants as grant (grant.name)}
			{@const Icon = KIND_ICON[grant.kind]}
			<div class="flex items-center gap-3 border-b border-border/30 px-3 py-2 last:border-b-0">
				<Icon class="size-3.5 shrink-0 text-muted-foreground" />
				<span class="min-w-0 flex-1 truncate text-xs">{grant.name}</span>
				{#if grant.expires}
					<span class="shrink-0 font-mono text-[10px] text-muted-foreground">{grant.expires}</span>
				{/if}
				<Badge variant="outline" class="shrink-0 text-[10px]">{ROLE_LABEL[grant.role]}</Badge>
			</div>
		{/each}
	</div>

	<ul class="flex flex-col gap-4">
		{#each ROLES_ASCENDING as role (role)}
			<li class="flex gap-3 text-sm">
				<span class="w-16 shrink-0 font-medium">{ROLE_LABEL[role]}</span>
				<span class="text-muted-foreground">{ROLE_HELP[role]}</span>
			</li>
		{/each}
	</ul>
</div>
