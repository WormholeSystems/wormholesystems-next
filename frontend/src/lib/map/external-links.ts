// The external sites a system links out to. Dotlan addresses systems and regions by name
// with spaces as underscores; zKillboard addresses everything by EVE id.

import type { Component } from 'svelte';

import CompassIcon from '@lucide/svelte/icons/compass';
import GlobeIcon from '@lucide/svelte/icons/globe';
import MapIcon from '@lucide/svelte/icons/map';

function dotlan(path: string): string {
	return `https://evemaps.dotlan.net/${path}`;
}

const underscore = (s: string) => s.replaceAll(' ', '_');

export function dotlanSystemUrl(name: string): string {
	return dotlan(`system/${underscore(name)}`);
}

export function dotlanRegionMapUrl(region: string, name: string): string {
	return dotlan(`map/${underscore(region)}/${underscore(name)}`);
}

/** Capital jump ranges from here, on Dotlan's range map. */
export function dotlanJumpRangeUrl(name: string): string {
	return dotlan(`range/Revelation,5/${underscore(name)}`);
}

export function zkillboardSystemUrl(solarSystemId: number): string {
	return `https://zkillboard.com/system/${solarSystemId}/`;
}

export function zkillboardConstellationUrl(constellationId: number): string {
	return `https://zkillboard.com/constellation/${constellationId}/`;
}

export function zkillboardRegionUrl(regionId: number): string {
	return `https://zkillboard.com/region/${regionId}/`;
}

export interface SystemLink {
	label: string;
	href: string;
	icon: Component<{ class?: string }>;
}

export interface SystemLinkGroup {
	label: string;
	/** Shown by the bits-ui menu; the on-map chrome menu stays text-only. */
	favicon: string;
	links: SystemLink[];
}

/** The External submenu's contents, shared by both menu chromes. */
export function systemLinkGroups(system: {
	solarSystemId: number;
	name: string;
	region: string;
	regionId: number;
	constellationId: number;
	isWormhole: boolean;
}): SystemLinkGroup[] {
	const dotlanLinks: SystemLink[] = [
		{ label: 'System', href: dotlanSystemUrl(system.name), icon: GlobeIcon },
		{ label: 'Region Map', href: dotlanRegionMapUrl(system.region, system.name), icon: MapIcon },
	];
	if (!system.isWormhole) {
		dotlanLinks.push({
			label: 'Jump Range',
			href: dotlanJumpRangeUrl(system.name),
			icon: CompassIcon,
		});
	}
	return [
		{ label: 'Dotlan', favicon: 'https://evemaps.dotlan.net/favicon.ico', links: dotlanLinks },
		{
			label: 'zKillboard',
			favicon: 'https://zkillboard.com/favicon.ico',
			links: [
				{ label: 'System', href: zkillboardSystemUrl(system.solarSystemId), icon: GlobeIcon },
				{
					label: 'Constellation',
					href: zkillboardConstellationUrl(system.constellationId),
					icon: CompassIcon,
				},
				{ label: 'Region', href: zkillboardRegionUrl(system.regionId), icon: MapIcon },
			],
		},
	];
}
