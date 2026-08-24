import { describe, expect, it } from 'vitest';

import { layoutMode } from './layout-mode';

describe('layoutMode', () => {
	it('follows the map when there is no override', () => {
		expect(layoutMode({ layout: 'tree', allow_layout_override: true }, null)).toBe('tree');
		expect(layoutMode({ layout: 'manual', allow_layout_override: true }, undefined)).toBe('manual');
	});

	it('honors the viewer override only when the map allows it', () => {
		expect(layoutMode({ layout: 'tree', allow_layout_override: true }, 'manual')).toBe('manual');
		expect(layoutMode({ layout: 'tree', allow_layout_override: false }, 'manual')).toBe('tree');
	});

	it('ignores an override it does not recognise', () => {
		expect(layoutMode({ layout: 'tree', allow_layout_override: true }, 'spiral')).toBe('tree');
	});

	it('is manual before the map arrives', () => {
		expect(layoutMode(null, 'tree')).toBe('manual');
	});
});
