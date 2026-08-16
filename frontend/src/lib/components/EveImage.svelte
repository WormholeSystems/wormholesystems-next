<script lang="ts">
	// An image from CCP's image server. Pass the entity id and a class for sizing/shape.
	const paths = {
		character: 'characters',
		corporation: 'corporations',
		alliance: 'alliances',
		type: 'types'
	} as const;
	const variants = {
		character: 'portrait',
		corporation: 'logo',
		alliance: 'logo',
		type: 'icon'
	} as const;

	let {
		kind,
		id,
		class: cls = '',
		title
	}: {
		kind: keyof typeof paths;
		id: number;
		class?: string;
		title?: string;
	} = $props();
</script>

<img
	src="https://images.evetech.net/{paths[kind]}/{id}/{variants[kind]}?size=64"
	class={cls}
	{title}
	alt=""
	loading="lazy"
/>
