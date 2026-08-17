// The sidebar's panel registry. Order and visibility are per user per map; an empty
// stored value means "the built-in layout", so a map nobody has customised renders
// exactly as it always did.

export type PanelId = 'navigation' | 'system-info' | 'threat' | 'signatures' | 'notes';

export interface PanelMeta {
	id: PanelId;
	label: string;
	/// Panels about the active system are hidden entirely when nothing is selected.
	needsSystem: boolean;
}

export const PANELS: PanelMeta[] = [
	{ id: 'navigation', label: 'Navigation', needsSystem: false },
	{ id: 'system-info', label: 'System', needsSystem: true },
	{ id: 'threat', label: 'Threat', needsSystem: true },
	{ id: 'signatures', label: 'Signatures', needsSystem: true },
	{ id: 'notes', label: 'Notes', needsSystem: true }
];

const DEFAULT_ORDER = PANELS.map((p) => p.id);

function isPanelId(v: string): v is PanelId {
	return DEFAULT_ORDER.includes(v as PanelId);
}

/**
 * The panels to render, in order. Stored ids come first in their saved order; anything
 * the stored order never mentioned (a panel added after the user last saved) keeps its
 * built-in position at the end, so a new panel appears rather than silently vanishing.
 */
export function visiblePanels(order: string[], hidden: string[]): PanelMeta[] {
	const saved = order.filter(isPanelId);
	const rest = DEFAULT_ORDER.filter((id) => !saved.includes(id));
	const hiddenSet = new Set(hidden);
	return [...saved, ...rest]
		.filter((id) => !hiddenSet.has(id))
		.map((id) => PANELS.find((p) => p.id === id)!);
}

/** Move `id` one step through the visible order, returning the full order to persist. */
export function reorder(order: string[], hidden: string[], id: PanelId, delta: -1 | 1): PanelId[] {
	const current = visiblePanels(order, hidden).map((p) => p.id);
	const from = current.indexOf(id);
	const to = from + delta;
	if (from < 0 || to < 0 || to >= current.length) return current;
	current.splice(to, 0, ...current.splice(from, 1));
	// Hidden panels keep a place in the saved order so unhiding restores them sensibly.
	return [...current, ...DEFAULT_ORDER.filter((p) => !current.includes(p))];
}
