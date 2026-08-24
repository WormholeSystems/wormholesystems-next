// The guided tour that follows the introduction: each step spotlights one part of the
// map screen and says what it is for. Steps anchor to testids, so the tour and the e2e
// suite lean on the same handles; a step whose anchor is not on screen (a watcher has
// no settings link) is skipped.

export interface TourStep {
	/** CSS selector for the element the step spotlights. */
	target: string;
	title: string;
	body: string;
}

export const TOUR_STEPS: TourStep[] = [
	{
		target: '[data-testid="map-canvas"]',
		title: 'The chain',
		body:
			'Every system lives here. Drag from one to another to connect them, right-click one ' +
			'for its menu, and paste a signature scan anywhere to fill in what you scanned.',
	},
	{
		target: '[data-testid="panel-grid"]',
		title: 'The panels',
		body:
			'Signatures, routes, intel and pilots, each in its own tile. Unlock the layout to ' +
			'rearrange, resize or hide them; the arrangement is yours alone.',
	},
	{
		target: '[data-testid="palette-trigger"]',
		title: 'Search everything',
		body:
			'The palette finds any system, on the map or off it, and places it with one pick. ' +
			'Cmd+K (Ctrl+K) opens it from anywhere.',
	},
	{
		target: '[data-testid="tracking-toggle"]',
		title: 'Location sharing',
		body:
			'The eye is your tracking switch: on, the map follows your pilots and maps jumps as ' +
			'you make them. The controls beside it tune what gets tracked and shown.',
	},
	{
		target: '[data-testid="history-controls"]',
		title: 'Undo, redo, history',
		body:
			'Every change lands in a shared history. Undo and redo step through it, and the ' +
			'clock opens the full timeline to jump anywhere in it.',
	},
	{
		target: '[data-testid="layout-toggle"]',
		title: 'Arrange the panels',
		body:
			'Unlocks the panel grid: drag tiles around, resize them, and hide what you do not ' +
			'use. The arrangement is saved for you alone.',
	},
	{
		target: '[data-testid="settings-link"]',
		title: 'Bring your corp',
		body:
			'Map settings hold access control: invite corporation members, hand out roles, and ' +
			'mint read-only share links. Everything from the walkthrough lives there too.',
	},
];

export interface Rect {
	x: number;
	y: number;
	width: number;
	height: number;
}

/** The spotlight's frame: the anchor plus breathing room, kept inside the viewport. */
export function spotlightRect(target: Rect, pad: number, viewport: { w: number; h: number }): Rect {
	const x = Math.max(0, target.x - pad);
	const y = Math.max(0, target.y - pad);
	return {
		x,
		y,
		width: Math.min(viewport.w, x + target.width + 2 * pad) - x,
		height: Math.min(viewport.h, y + target.height + 2 * pad) - y,
	};
}

/**
 * Where the explaining card goes: below the spotlight when there is room, above it
 * otherwise, and beside it when neither fits; always clamped to the viewport.
 */
export function cardPosition(
	spot: Rect,
	card: { w: number; h: number },
	viewport: { w: number; h: number },
	gap = 12,
): { x: number; y: number } {
	const x = Math.min(Math.max(8, spot.x), viewport.w - card.w - 8);
	if (spot.y + spot.height + gap + card.h <= viewport.h) {
		return { x, y: spot.y + spot.height + gap };
	}
	if (spot.y - gap - card.h >= 0) {
		return { x, y: spot.y - gap - card.h };
	}
	const beside =
		spot.x + spot.width + gap + card.w <= viewport.w
			? spot.x + spot.width + gap
			: Math.max(8, spot.x - gap - card.w);
	return { x: beside, y: Math.min(Math.max(8, spot.y), viewport.h - card.h - 8) };
}
