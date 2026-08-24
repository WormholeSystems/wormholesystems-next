/**
 * How this map is placed for one viewer: the map's own mode, unless it hands the choice
 * over and the viewer has made one.
 */
export function layoutMode(
	map: { layout: string; allow_layout_override: boolean } | null,
	override: string | null | undefined,
): 'manual' | 'tree' {
	if (!map) return 'manual';
	if (map.allow_layout_override && (override === 'manual' || override === 'tree')) return override;
	return map.layout === 'tree' ? 'tree' : 'manual';
}
