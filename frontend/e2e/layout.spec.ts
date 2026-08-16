import { expect, gotoApp, test } from './fixtures';

// Arranging the side panels: order and visibility are per user per map, so they have to
// survive a reload.

const J122515 = 31001882;

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

test('panels can be hidden, restored, and reordered', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Layout');
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J122515, x: 200, y: 200, alias: null }
	});
	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);

	// The controls only exist in edit mode.
	await expect(page.getByTestId('panel-controls-notes')).toHaveCount(0);
	await page.getByTestId('layout-toggle').click();
	await expect(page.getByTestId('panel-controls-notes')).toBeVisible();

	await expect(page.getByTestId('notes-card')).toBeVisible();
	await page.getByTestId('hide-notes').click();
	await expect(page.getByTestId('notes-card')).toHaveCount(0);

	// Hiding persists.
	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	await expect(page.getByTestId('notes-card')).toHaveCount(0);

	// And the hidden panel can be brought back from the edit-mode tray.
	await page.getByTestId('layout-toggle').click();
	await page.getByTestId('show-notes').click();
	await expect(page.getByTestId('notes-card')).toBeVisible();
});

test('reordering moves a panel and survives a reload', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Reorder');
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J122515, x: 200, y: 200, alias: null }
	});
	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);

	// Order is read off the DOM, so this asserts what the user actually sees.
	const ids = () =>
		page.getByTestId('sidebar').locator('[data-testid$="-card"], [data-testid="system-info"]');
	await expect(ids().first()).toHaveAttribute('data-testid', 'navigation-card');

	await page.getByTestId('layout-toggle').click();
	await page
		.getByTestId('panel-controls-navigation')
		.getByRole('button', { name: 'Move down' })
		.click();
	await expect(ids().first()).toHaveAttribute('data-testid', 'system-info');

	// The new order is stored per user per map, not just held in the page.
	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	await expect(ids().first()).toHaveAttribute('data-testid', 'system-info');
	await expect(ids().nth(1)).toHaveAttribute('data-testid', 'navigation-card');
});
