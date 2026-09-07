// The background picture behind a viewer's map: what the server accepts, and the two ways
// it can sit. Shared by the on-map popover and the display settings page.
import type { BackgroundMode } from '$lib/api/types/BackgroundMode';

export const MAX_IMAGE_BYTES = 8 * 1024 * 1024;

export const ACCEPTED_IMAGE_TYPES = 'image/png,image/jpeg,image/gif,image/webp';

export const BACKGROUND_MODES: readonly {
	value: BackgroundMode;
	label: string;
	hint: string;
}[] = [
	{
		value: 'grid',
		label: 'Moves with the map',
		hint: 'Spans the grid, so it pans and zooms with the systems',
	},
	{
		value: 'viewport',
		label: 'Fills the panel',
		hint: 'Fills the visible panel and stays put while you navigate',
	},
];

export const BACKGROUND_MODE_VALUES = BACKGROUND_MODES.map((m) => m.value);

/** Why a picked file cannot be sent, or null when it can. */
export function rejectImage(file: File): string | null {
	if (!file.type.startsWith('image/')) return 'That is not an image.';
	if (file.size > MAX_IMAGE_BYTES) return 'That image is over 8 MiB. Shrink it first.';
	return null;
}
