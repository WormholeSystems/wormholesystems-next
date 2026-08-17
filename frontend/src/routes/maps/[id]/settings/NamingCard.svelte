<script lang="ts">
	// How the map names its chain: the alias sequence, and the three bookmark formats.
	//
	// The formats are opaque token strings, so each one previews against a worked example
	// as you type. Without that you only find out you wrote `{sig}` where you meant
	// `{wh}` after the bookmark is already in the folder.
	import { untrack } from 'svelte';

	import { guessNextAlias } from '$lib/alias';
	import type { AliasScheme } from '$lib/alias';
	import {
		BOOKMARK_TOKENS,
		DEFAULT_FORMAT_KSPACE,
		DEFAULT_FORMAT_RETURN,
		DEFAULT_FORMAT_WORMHOLE,
		DEFAULT_IGNORED_ALIAS,
		renderBookmark
	} from '$lib/bookmark';
	import type { BookmarkToken } from '$lib/bookmark';
	import type { MapNaming } from '$lib/api/types/MapNaming';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import * as ToggleGroup from '$lib/components/ui/toggle-group';

	let {
		naming,
		disabled = false,
		onsave
	}: {
		naming: MapNaming;
		disabled?: boolean;
		onsave: (naming: MapNaming) => void;
	} = $props();

	const initial = untrack(() => JSON.stringify(naming));
	let draft = $state<MapNaming>(JSON.parse(initial));

	// Re-seed when the saved value changes underneath (a reload, or another manager's
	// edit), without clobbering what is being typed in between.
	const savedKey = $derived(JSON.stringify(naming));
	let lastSeeded = $state(initial);
	$effect(() => {
		if (savedKey !== lastSeeded) {
			lastSeeded = savedKey;
			draft = { ...naming };
		}
	});

	const dirty = $derived(JSON.stringify(draft) !== savedKey);

	const scheme = $derived(draft.alias_scheme as AliasScheme);

	/// The first three children of a chain, so switching scheme shows the difference.
	const aliasPreview = $derived.by(() => {
		const taken: string[] = [];
		for (let i = 0; i < 3; i++) {
			taken.push(guessNextAlias('', taken, { scheme, ignoredAlias: draft.ignored_alias }));
		}
		const child = guessNextAlias(taken[0], taken, { scheme, ignoredAlias: draft.ignored_alias });
		return [...taken, child].join(', ');
	});

	const EXAMPLE: Record<BookmarkToken, string> = {
		alias: '1a',
		sig: 'ABC',
		class: 'C5',
		name: 'J155207',
		region: 'E-R00028',
		occupier: 'Hard Knocks',
		size: 'MD',
		wh: 'H296',
		mass: 'crit',
		life: 'EOL'
	};

	const KSPACE_EXAMPLE: Record<BookmarkToken, string> = {
		...EXAMPLE,
		alias: '1b',
		class: 'HS',
		name: 'Jita',
		region: 'The Forge',
		occupier: '',
		wh: 'B041'
	};

	const FORMATS = [
		{
			key: 'bookmark_wormhole' as const,
			label: 'Wormhole',
			help: 'A hole leading further into wormhole space.',
			fallback: DEFAULT_FORMAT_WORMHOLE,
			example: EXAMPLE
		},
		{
			key: 'bookmark_kspace' as const,
			label: 'K-space',
			help: 'A hole leading out to known space.',
			fallback: DEFAULT_FORMAT_KSPACE,
			example: KSPACE_EXAMPLE
		},
		{
			key: 'bookmark_return' as const,
			label: 'Return',
			help: 'The hole back the way you came.',
			fallback: DEFAULT_FORMAT_RETURN,
			example: EXAMPLE
		}
	];
</script>

<Card.Root>
	<Card.Header>
		<Card.Title>Chain naming</Card.Title>
		<Card.Description>
			How aliases and bookmarks are generated for everyone on this map.
		</Card.Description>
	</Card.Header>
	<Card.Content>
		<Field.FieldGroup>
			<Field.Field>
				<Field.FieldLabel>Alias scheme</Field.FieldLabel>
				<ToggleGroup.Root
					type="single"
					variant="outline"
					value={draft.alias_scheme}
					{disabled}
					onValueChange={(value) => value && (draft.alias_scheme = value)}
					data-testid="alias-scheme"
				>
					<ToggleGroup.Item value="numeric">Numeric</ToggleGroup.Item>
					<ToggleGroup.Item value="alphabetical">Alphabetical</ToggleGroup.Item>
				</ToggleGroup.Root>
				<Field.FieldDescription>
					Chains start <span class="font-mono" data-testid="alias-preview">{aliasPreview}</span>
					{#if scheme === 'alphabetical'}
						. H, L, N and P are reserved for exits to known space.
					{/if}
				</Field.FieldDescription>
			</Field.Field>

			<Field.Field>
				<Field.FieldLabel for="ignored-alias">Home alias</Field.FieldLabel>
				<Input
					id="ignored-alias"
					bind:value={draft.ignored_alias}
					placeholder={DEFAULT_IGNORED_ALIAS}
					{disabled}
					data-testid="ignored-alias"
				/>
				<Field.FieldDescription>
					Sits outside the chain: its holes start a fresh sequence, and anything pointing back
					at it is bookmarked as a way home.
				</Field.FieldDescription>
			</Field.Field>

			<Field.FieldSet>
				<Field.FieldLegend>Bookmark formats</Field.FieldLegend>
				<Field.FieldDescription>
					Available: {BOOKMARK_TOKENS.map((t) => `{${t}}`).join(' ')}. Empty ones drop out.
				</Field.FieldDescription>
				<Field.FieldGroup>
					{#each FORMATS as format (format.key)}
						<Field.Field>
							<Field.FieldLabel for={format.key}>{format.label}</Field.FieldLabel>
							<Input
								id={format.key}
								bind:value={draft[format.key]}
								placeholder={format.fallback}
								{disabled}
								class="font-mono"
								data-testid={format.key}
							/>
							<Field.FieldDescription>
								{format.help}
								<span class="font-mono text-foreground" data-testid="{format.key}-preview">
									{renderBookmark(draft[format.key] || format.fallback, format.example)}
								</span>
							</Field.FieldDescription>
						</Field.Field>
					{/each}
				</Field.FieldGroup>
			</Field.FieldSet>
		</Field.FieldGroup>
	</Card.Content>
	<Card.Footer class="justify-end gap-2">
		<Button variant="ghost" disabled={disabled || !dirty} onclick={() => (draft = { ...naming })}>
			Reset
		</Button>
		<Button
			variant="outline"
			disabled={disabled || !dirty}
			onclick={() => onsave({ ...draft })}
			data-testid="save-naming">Save</Button
		>
	</Card.Footer>
</Card.Root>
