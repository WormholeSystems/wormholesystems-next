import { describe, expect, it } from 'vitest';

import {
	dotlanJumpRangeUrl,
	dotlanRegionMapUrl,
	dotlanSystemUrl,
	zkillboardConstellationUrl,
	zkillboardRegionUrl,
	zkillboardSystemUrl,
} from './external-links';

describe('dotlan urls', () => {
	it('addresses a system by name', () => {
		expect(dotlanSystemUrl('Jita')).toBe('https://evemaps.dotlan.net/system/Jita');
	});

	it('replaces every space with an underscore', () => {
		expect(dotlanRegionMapUrl('The Forge', 'New Caldari')).toBe(
			'https://evemaps.dotlan.net/map/The_Forge/New_Caldari',
		);
	});

	it('ranges from the system on a Revelation at jump 5', () => {
		expect(dotlanJumpRangeUrl('Amamake')).toBe(
			'https://evemaps.dotlan.net/range/Revelation,5/Amamake',
		);
	});
});

describe('zkillboard urls', () => {
	it('addresses system, constellation and region by id, with a trailing slash', () => {
		expect(zkillboardSystemUrl(30000142)).toBe('https://zkillboard.com/system/30000142/');
		expect(zkillboardConstellationUrl(20000020)).toBe(
			'https://zkillboard.com/constellation/20000020/',
		);
		expect(zkillboardRegionUrl(10000002)).toBe('https://zkillboard.com/region/10000002/');
	});
});
