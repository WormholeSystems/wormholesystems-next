import { describe, expect, it } from 'vitest';

import type { MapSystemView } from '$lib/api/types/MapSystemView';
import { deepLinkTarget, systemParamUrl } from './deep-link';

const systems = [
	{ kind: 'system', id: 1, solar_system_id: 30000142 },
	{ kind: 'ghost', id: 2 },
] as MapSystemView[];

describe('deepLinkTarget', () => {
	it('resolves the param to its node, ignoring ghosts and strangers', () => {
		expect(deepLinkTarget(systems, '30000142')).toBe(1);
		expect(deepLinkTarget(systems, '30009999')).toBeNull();
		expect(deepLinkTarget(systems, null)).toBeNull();
		expect(deepLinkTarget(systems, 'jita')).toBeNull();
	});
});

describe('systemParamUrl', () => {
	it('writes the param without touching the rest of the url', () => {
		const url = systemParamUrl(new URL('https://x.test/maps/7?panel=sigs'), 30000142);
		expect(url?.searchParams.get('system')).toBe('30000142');
		expect(url?.searchParams.get('panel')).toBe('sigs');
	});

	it('answers null when the param is already current', () => {
		expect(systemParamUrl(new URL('https://x.test/maps/7?system=30000142'), 30000142)).toBeNull();
	});
});
