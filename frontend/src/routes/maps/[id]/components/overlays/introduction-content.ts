// The introduction walkthrough's content and derivations, out of the dialog so the
// component is wiring and markup.

import EyeIcon from '@lucide/svelte/icons/eye';
import RouteIcon from '@lucide/svelte/icons/route';
import ShieldIcon from '@lucide/svelte/icons/shield';
import SignatureIcon from '@lucide/svelte/icons/scan-line';
import TagIcon from '@lucide/svelte/icons/tag';
import type { Component } from 'svelte';

import type { MapUserSettings } from '$lib/api/types/MapUserSettings';

export const INTRO_STEPS = [
	{
		title: 'Welcome to the map',
		blurb: 'A minute of setup, and it maps the chain as you fly it.',
	},
	{
		title: 'Grant permissions',
		blurb: 'What the map may read from your EVE client. All optional.',
	},
	{ title: 'Choose what it does', blurb: 'How much of the mapping you want done for you.' },
	{ title: 'Ready to fly', blurb: 'Here is where everything ended up.' },
];

export const INTRO_OPENING: { icon: Component; text: string }[] = [
	{ icon: ShieldIcon, text: 'The EVE permissions the map can use' },
	{ icon: EyeIcon, text: 'Whether it may follow you around' },
	{ icon: RouteIcon, text: 'How much of the mapping it does for you' },
];

export function introSummary(
	settings: MapUserSettings | null,
	granted: number,
	total: number,
): { label: string; value: string; good: boolean }[] {
	return [
		{
			label: 'Permissions',
			value: granted === total ? 'All granted' : `${granted} of ${total} granted`,
			good: granted === total,
		},
		{
			label: 'Location sharing',
			value: settings?.tracking_allowed ? 'On' : 'Off',
			good: settings?.tracking_allowed ?? false,
		},
		{
			label: 'Signature prompt',
			value: settings?.tracking_allowed && settings?.prompt_for_signature ? 'On' : 'Off',
			good: (settings?.tracking_allowed && settings?.prompt_for_signature) ?? false,
		},
	];
}

export interface IntroToggle {
	key: 'tracking_allowed' | 'prompt_for_signature' | 'suggest_alias';
	icon: Component;
	name: string;
	body: string;
	value: boolean;
	enabled: boolean;
	blocked: string;
}

export function introToggles(
	settings: MapUserSettings | null,
	hasLocation: boolean,
): IntroToggle[] {
	return [
		{
			key: 'tracking_allowed',
			icon: EyeIcon,
			name: 'Share my location on this map',
			body:
				'The map follows you between systems, shows you to everyone else here, and measures ' +
				'distances from where you are. Revocable at any time.',
			value: settings?.tracking_allowed ?? false,
			enabled: hasLocation,
			blocked: 'Needs the character location permission.',
		},
		{
			key: 'prompt_for_signature',
			icon: SignatureIcon,
			name: 'Ask which signature I jumped',
			body:
				'When you arrive somewhere new, the map asks which signature the hole was and links ' +
				'it, instead of leaving an unnamed connection behind.',
			value: settings?.prompt_for_signature ?? true,
			enabled: settings?.tracking_allowed ?? false,
			blocked: 'Needs location sharing.',
		},
		{
			key: 'suggest_alias',
			icon: TagIcon,
			name: 'Name new systems for me',
			body:
				"Fills in the next alias from the chain's naming scheme, so holes are named the " +
				'same way by everyone.',
			value: settings?.suggest_alias ?? true,
			enabled: settings?.tracking_allowed ?? false,
			blocked: 'Needs location sharing.',
		},
	];
}
