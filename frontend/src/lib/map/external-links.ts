// The external sites a system links out to. Dotlan addresses systems and regions by name
// with spaces as underscores; zKillboard addresses everything by EVE id.

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
