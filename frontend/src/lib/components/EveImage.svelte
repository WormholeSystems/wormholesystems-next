<script lang="ts">
	// An image from CCP's image server: the one primitive for every entity kind. Pass the
	// entity id, a class for sizing/shape, and optionally the server resolution (a power
	// of two; keep it at or above 2x the displayed size for crisp rendering). Factions
	// are served from the corporations endpoint (legacy behavior).
	const paths = {
		character: 'characters',
		corporation: 'corporations',
		alliance: 'alliances',
		faction: 'corporations',
		type: 'types'
	} as const;
	const variants = {
		character: 'portrait',
		corporation: 'logo',
		alliance: 'logo',
		faction: 'logo',
		type: 'icon'
	} as const;

	let {
		kind,
		id,
		class: cls = '',
		title,
		size = 64
	}: {
		kind: keyof typeof paths;
		id: number;
		class?: string;
		title?: string;
		size?: 32 | 64 | 128 | 256 | 512;
	} = $props();
</script>

<img
	src="https://images.evetech.net/{paths[kind]}/{id}/{variants[kind]}?size={size}"
	class={cls}
	{title}
	alt=""
	loading="lazy"
/>
