import { describe, expect, it } from 'vitest';

import { cardPosition, spotlightRect } from './tour';

const VIEWPORT = { w: 1280, h: 720 };
const CARD = { w: 320, h: 190 };

describe('spotlightRect', () => {
	it('pads the anchor without leaving the viewport', () => {
		expect(spotlightRect({ x: 100, y: 100, width: 200, height: 50 }, 8, VIEWPORT)).toEqual({
			x: 92,
			y: 92,
			width: 216,
			height: 66,
		});
		const clamped = spotlightRect({ x: 0, y: 0, width: 1280, height: 40 }, 8, VIEWPORT);
		expect(clamped.x).toBe(0);
		expect(clamped.width).toBe(1280);
	});
});

describe('cardPosition', () => {
	it('sits below the spotlight when there is room', () => {
		const at = cardPosition({ x: 100, y: 100, width: 200, height: 50 }, CARD, VIEWPORT);
		expect(at).toEqual({ x: 100, y: 162 });
	});

	it('flips above a spotlight near the bottom edge', () => {
		const at = cardPosition({ x: 100, y: 640, width: 200, height: 60 }, CARD, VIEWPORT);
		expect(at.y).toBe(640 - 12 - CARD.h);
	});

	it('moves beside a spotlight that fills the whole height', () => {
		const at = cardPosition({ x: 0, y: 0, width: 300, height: 720 }, CARD, VIEWPORT);
		expect(at.x).toBe(300 + 12);
		expect(at.y).toBe(8);
	});

	it('never leaves the right edge', () => {
		const at = cardPosition({ x: 1200, y: 100, width: 60, height: 30 }, CARD, VIEWPORT);
		expect(at.x + CARD.w).toBeLessThanOrEqual(VIEWPORT.w - 8);
	});
});
