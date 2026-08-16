<script lang="ts">
	import { page } from '$app/state';

	const param = (key: string) => page.url.searchParams.get(key);
</script>

<div class="max-w-2xl">
	<h1 class="text-lg font-semibold tracking-tight">Profile</h1>
	{#if param('name') === null}
		<p class="mt-4 text-sm text-muted-foreground">
			Not logged in. <a href="/login" class="text-foreground underline">Log in</a>.
		</p>
	{:else}
		<p class="mt-4 text-sm text-muted-foreground">
			Logged in as <span class="font-medium text-foreground">{param('name')}</span>.
		</p>

		<h2 class="mt-6 text-xs font-medium uppercase tracking-wider text-muted-foreground">
			Affiliations
		</h2>
		<ul class="mt-2 space-y-0.5 text-sm text-foreground">
			{#if param('corporation')}<li>Corporation: {param('corporation')}</li>{/if}
			{#if param('alliance')}<li>Alliance: {param('alliance')}</li>{/if}
			{#if param('faction')}<li>Faction: {param('faction')}</li>{/if}
		</ul>

		<h2 class="mt-6 text-xs font-medium uppercase tracking-wider text-muted-foreground">
			Granted scopes
		</h2>
		{#if param('scopes')}
			<ul class="mt-2 space-y-0.5 font-mono text-xs text-foreground">
				{#each param('scopes')!.split(' ') as scope (scope)}
					<li>{scope}</li>
				{/each}
			</ul>
		{:else}
			<p class="mt-2 text-sm text-muted-foreground">None.</p>
		{/if}
	{/if}
</div>
