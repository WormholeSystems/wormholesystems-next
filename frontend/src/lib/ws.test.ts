import { describe, expect, it } from 'vitest';

import { parseFrame } from './ws';

describe('parseFrame', () => {
	it('accepts an event-shaped frame', () => {
		expect(parseFrame('{"type":"map_updated","map_id":1}')).toEqual({
			type: 'map_updated',
			map_id: 1,
		});
	});

	it('refuses frames that are not events at all', () => {
		expect(parseFrame('not json')).toBeNull();
		expect(parseFrame('42')).toBeNull();
		expect(parseFrame('null')).toBeNull();
		expect(parseFrame('{"kind":"other"}')).toBeNull();
	});
});
