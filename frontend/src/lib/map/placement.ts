// The two placement modes, with every surface's copy: `hint` is the settings page's short
// line, `body` the introduction dialog's fuller one.

import WaypointsIcon from '@lucide/svelte/icons/waypoints';
import WorkflowIcon from '@lucide/svelte/icons/workflow';
import type { Component } from 'svelte';

import type { MapLayout } from '$lib/api/types/MapLayout';

export interface PlacementOption {
	value: MapLayout;
	label: string;
	hint: string;
	body: string;
	icon: Component;
}

export const PLACEMENTS: PlacementOption[] = [
	{
		value: 'manual',
		label: 'Custom placement',
		hint: 'Everyone drags the chain into shape',
		body: 'You drag the systems into shape, and they stay where you put them.',
		icon: WaypointsIcon,
	},
	{
		value: 'tree',
		label: 'Automatic placement',
		hint: 'Drawn as a tree from the connections',
		body: 'The map draws the chain as a tree, and nobody has to tidy it.',
		icon: WorkflowIcon,
	},
];
