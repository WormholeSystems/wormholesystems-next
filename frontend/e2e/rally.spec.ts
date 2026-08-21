import { createdId, expect, gotoApp, test } from './fixtures';

// The rally badge: where the fleet is forming up, and how far that is from the staging
// system. The count is the map's own, home to rally, so everyone reads the same number.

const JITA = 30000142;
const PERIMETER = 30000144; // one gate from Jita

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return await createdId(res);
}

async function addSystem(
	api: import('@playwright/test').APIRequestContext,
	mapId: number,
	solarSystemId: number,
	x: number,
	y: number,
) {
	const res = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: solarSystemId, x, y, alias: null },
	});
	expect(res.ok()).toBe(true);
	return await createdId(res);
}

test('the rally badge names the system and counts the jumps home', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Rally');
	const home = await addSystem(api, mapId, JITA, 200, 200);
	const rally = await addSystem(api, mapId, PERIMETER, 500, 200);

	await gotoApp(page, `/maps/${mapId}`);
	// Nothing to form up on yet, so nothing is drawn.
	await expect(page.getByTestId('rally-badge')).toHaveCount(0);

	for (const [path, id] of [
		['set-home', home],
		['set-rally', rally],
	] as const) {
		const res = await api.post(`/api/maps/${mapId}/systems/${path}`, {
			data: { map_id: mapId, map_solar_system_id: id, value: true },
		});
		expect(res.ok()).toBe(true);
	}

	const badge = page.getByTestId('rally-badge');
	await expect(badge).toBeVisible();
	await expect(badge).toContainText('Rally point');
	await expect(badge).toContainText('Perimeter');

	// Jita and Perimeter are one gate apart, and the route is reachable without the map
	// having an edge for it: k-space routes through the stargate graph.
	const jumps = page.getByTestId('rally-jumps');
	await expect(jumps).toHaveText('1j');

	// The count opens the route it counted.
	await jumps.click();
	const route = page.getByTestId('route-list');
	await expect(route).toBeVisible();
	await expect(route).toContainText('Perimeter');

	await api.delete(`/api/maps/${mapId}`);
});
