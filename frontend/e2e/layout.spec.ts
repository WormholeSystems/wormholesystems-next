import { expect, gotoApp, test } from './fixtures';

// Arranging the map page. The layout is per user per map, so everything here has to
// survive a reload, and the map canvas is a tile like any other.

const J122515 = 31001882;

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

/** Position and size of a tile, read off the DOM. */
async function box(page: import('@playwright/test').Page, panel: string) {
	const el = page.locator(`[data-testid="panel-tile"][data-panel="${panel}"]`);
	return {
		x: Number(await el.getAttribute('data-x')),
		y: Number(await el.getAttribute('data-y')),
		w: Number(await el.getAttribute('data-w')),
		h: Number(await el.getAttribute('data-h'))
	};
}

/** Drag a tile by whole grid cells using its edit-mode shield. */
async function dragTile(
	page: import('@playwright/test').Page,
	panel: string,
	dx: number,
	dy: number
) {
	const shield = page.locator(`[data-testid="tile-shield"][data-panel="${panel}"]`);
	// The grid is taller than the window, so a tile can start below the fold; mouse
	// coordinates are viewport-relative.
	await shield.scrollIntoViewIfNeeded();
	const from = await shield.boundingBox();
	if (!from) throw new Error(`no shield for ${panel}`);
	const startX = from.x + Math.min(from.width / 2, 60);
	const startY = from.y + 20;
	await page.mouse.move(startX, startY);
	await page.mouse.down();
	// Two steps so the gesture clears its 4px hysteresis before the real move.
	await page.mouse.move(startX + 8, startY + 8);
	await page.mouse.move(startX + dx, startY + dy, { steps: 8 });
	await page.mouse.up();
	// Tiles preview the gesture while it runs; wait for it to commit before reading
	// positions, or an intermediate step gets mistaken for the result.
	await expect(page.getByTestId('panel-grid')).toHaveAttribute('data-dragging', 'false');
}

test('the map is a tile, and the grid follows the window width', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Grid');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);

	await expect(page.getByTestId('panel-grid')).toHaveAttribute('data-breakpoint', 'lg');
	// The canvas is one of the tiles, not a fixed sidebar neighbour.
	await expect(page.locator('[data-testid="panel-tile"][data-panel="map"]')).toBeVisible();
	await expect(page.getByTestId('map-canvas')).toBeVisible();

	// A narrow window switches to a different arrangement.
	await page.setViewportSize({ width: 800, height: 1000 });
	await expect(page.getByTestId('panel-grid')).toHaveAttribute('data-breakpoint', 'sm');
});

test('dragging a tile moves it and the move survives a reload', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Drag');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);

	const before = await box(page, 'notes');
	await page.getByTestId('layout-toggle').click();
	await expect(page.getByTestId('layout-toolbar')).toBeVisible();

	// Drag notes a long way left; it should end in a different column.
	await dragTile(page, 'notes', -700, 0);
	const after = await box(page, 'notes');
	expect(after.x).toBeLessThan(before.x);

	await expect(page.getByTestId('layout-save')).toBeEnabled();
	await page.getByTestId('layout-save').click();
	await expect(page.getByTestId('layout-toolbar')).toHaveCount(0);

	await gotoApp(page, `/maps/${mapId}`);
	// Settings arrive after hydration, so this has to be a retrying assertion: reading
	// once can catch the built-in arrangement before the stored one lands.
	await expect(page.locator('[data-testid="panel-tile"][data-panel="notes"]')).toHaveAttribute(
		'data-x',
		String(after.x)
	);
});

test('arrow keys move a tile without a pointer', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Keyboard');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);
	await page.getByTestId('layout-toggle').click();

	const before = await box(page, 'notes');
	await page.locator('[data-testid="tile-shield"][data-panel="notes"]').focus();
	await page.keyboard.press('ArrowLeft');
	expect((await box(page, 'notes')).x).toBe(before.x - 1);

	// Shift resizes instead of moving.
	const width = (await box(page, 'notes')).w;
	await page.keyboard.press('Shift+ArrowRight');
	expect((await box(page, 'notes')).w).toBe(width + 1);
});

test('a resize will not go below the panel minimum', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Minimum');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);
	await page.getByTestId('layout-toggle').click();

	const shield = page.locator('[data-testid="tile-shield"][data-panel="notes"]');
	await shield.focus();
	// Shrink far past the minimum; it clamps rather than collapsing.
	for (let i = 0; i < 8; i++) await page.keyboard.press('Shift+ArrowLeft');
	expect((await box(page, 'notes')).w).toBe(2);
});

test('hiding a panel and adding it back from the library', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Hide');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	await page.getByTestId('layout-toggle').click();

	await page.locator('[data-testid="tile-hide"][data-panel="notes"]').click();
	await expect(page.locator('[data-testid="panel-tile"][data-panel="notes"]')).toHaveCount(0);
	await page.getByTestId('layout-save').click();
	// The toolbar closes once the save lands; navigating before that would race it.
	await expect(page.getByTestId('layout-toolbar')).toHaveCount(0);

	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	await expect(page.locator('[data-testid="panel-tile"][data-panel="notes"]')).toHaveCount(0);

	// The card library offers it back.
	await page.getByTestId('layout-toggle').click();
	await page.getByTestId('card-library').click();
	await page.getByTestId('add-notes').click();
	await expect(page.locator('[data-testid="panel-tile"][data-panel="notes"]')).toBeVisible();

	// The map can never be hidden; there would be nothing left to look at.
	await expect(page.locator('[data-testid="tile-hide"][data-panel="map"]')).toHaveCount(0);
});

test('discarding puts the arrangement back', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Discard');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);

	const before = await box(page, 'notes');
	await page.getByTestId('layout-toggle').click();
	await dragTile(page, 'notes', -700, 0);
	expect((await box(page, 'notes')).x).not.toBe(before.x);

	await page.getByTestId('layout-exit').click();
	await page.getByTestId('layout-discard').click();
	await expect(page.getByTestId('layout-toolbar')).toHaveCount(0);
	expect(await box(page, 'notes')).toEqual(before);
});

test('reset returns one breakpoint to the built-in arrangement', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Reset');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);

	const before = await box(page, 'notes');
	await page.getByTestId('layout-toggle').click();
	await dragTile(page, 'notes', -700, 0);
	expect((await box(page, 'notes')).x).not.toBe(before.x);

	await page.getByTestId('layout-more').click();
	await page.getByTestId('layout-reset').click();
	expect(await box(page, 'notes')).toEqual(before);
});

test('each breakpoint keeps its own arrangement', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Breakpoints');
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);
	await page.getByTestId('layout-toggle').click();

	// The desktop arrangement puts the map beside the panels; the phone one stacks.
	await expect(page.getByTestId('panel-grid')).toHaveAttribute('data-breakpoint', 'lg');
	const wide = await box(page, 'map');
	await page.getByTestId('breakpoint-xs').click();
	await expect(page.getByTestId('panel-grid')).toHaveAttribute('data-breakpoint', 'xs');
	const narrow = await box(page, 'map');
	expect(narrow.w).toBeLessThan(wide.w);
});

test('the canvas still pans after the map tile is resized', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Pan');
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J122515, x: 300, y: 200, alias: null }
	});
	await page.setViewportSize({ width: 1700, height: 1000 });
	await gotoApp(page, `/maps/${mapId}`);

	await page.getByTestId('layout-toggle').click();
	await page.locator('[data-testid="tile-shield"][data-panel="map"]').focus();
	await page.keyboard.press('Shift+ArrowLeft');
	await page.getByTestId('layout-save').click();
	await expect(page.getByTestId('layout-toolbar')).toHaveCount(0);

	// Middle-drag the canvas; the node must move with the pan.
	const node = page.getByTestId('system-node').first();
	const start = await node.boundingBox();
	const canvas = await page.getByTestId('map-canvas').boundingBox();
	if (!start || !canvas) throw new Error('missing geometry');
	await page.mouse.move(canvas.x + canvas.width / 2, canvas.y + canvas.height / 2);
	await page.mouse.down({ button: 'middle' });
	await page.mouse.move(canvas.x + canvas.width / 2 - 120, canvas.y + canvas.height / 2, {
		steps: 8
	});
	await page.mouse.up({ button: 'middle' });

	const moved = await node.boundingBox();
	expect(moved!.x).toBeLessThan(start.x);
});
