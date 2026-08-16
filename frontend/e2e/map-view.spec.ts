import { expect, gotoApp, test } from './fixtures';

// The interactive map canvas. Each test arranges its own map through the API and
// navigates straight to it; global teardown removes everything the e2e user owns.

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

test('search dialog adds a system to the map', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Search Map');
	await gotoApp(page, `/maps/${mapId}`);

	const canvas = page.getByTestId('map-canvas');
	await canvas.click({ button: 'right', position: { x: 500, y: 500 } });
	await page.getByRole('button', { name: 'Add solar system' }).click();

	const input = page.getByPlaceholder('Search for a system…');
	await expect(input).toBeVisible();
	await input.fill('jita');
	const result = page.locator('[data-slot="command-item"]', { hasText: 'Jita' });
	await expect(result).toBeVisible();
	await result.click();

	// The node appears on the canvas with its name and region, centered on the
	// right-click spot (grid-snapped).
	const node = page.getByTestId('system-node').filter({ hasText: 'Jita' });
	await expect(node).toBeVisible();
	await expect(node.getByText('The Forge')).toBeVisible();
	const pos = await node.evaluate((el) => ({
		left: parseFloat((el as HTMLElement).style.left),
		top: parseFloat((el as HTMLElement).style.top)
	}));
	// Click at (500, 500), node 180 wide / 40 tall → top-left near (410, 480).
	expect(Math.abs(pos.left - 410)).toBeLessThanOrEqual(40);
	expect(Math.abs(pos.top - 480)).toBeLessThanOrEqual(40);
});

test('node context menu sets the system status', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Status Map');
	const add = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: 30000142, x: 200, y: 200, alias: null }
	});
	expect(add.ok()).toBe(true);

	await gotoApp(page, `/maps/${mapId}`);
	const node = page.getByTestId('system-node').filter({ hasText: 'Jita' });
	await node.click({ button: 'right' });
	await page.getByTestId('status-subtrigger').hover();
	await page.getByTestId('status-submenu').getByRole('button', { name: 'Friendly' }).click();

	// The friendly status recolors the node border via its data-status channel.
	await expect(node).toHaveAttribute('data-status', 'friendly');
});
