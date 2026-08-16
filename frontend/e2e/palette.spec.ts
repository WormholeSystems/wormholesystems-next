import { expect, gotoApp, test } from './fixtures';
import { createIdentity, grantAccess } from './db';

// The Cmd+K palette: jump to a system already on the map, or add one that is not.

const J122515 = 31001882; // C5, Wolf-Rayet Star
const JITA = 30000142;

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

test('Cmd+K opens the palette and jumps to a placed system', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Palette');
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J122515, x: 200, y: 200, alias: 'Staging' }
	});
	await gotoApp(page, `/maps/${mapId}`);

	await page.keyboard.press('ControlOrMeta+k');
	await expect(page.getByTestId('palette-list')).toBeVisible();

	// The alias matches, not just the system name.
	await page.getByPlaceholder('System, alias, occupier or notes…').fill('Stag');
	const hit = page.getByTestId('palette-hit').first();
	await expect(hit).toContainText('J122515');
	await hit.click();

	// Picking a hit activates the system, which the deep link reflects.
	await expect(page).toHaveURL(new RegExp(`system=${J122515}`));
});

test('an off-map system can be added straight from the palette', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E PaletteAdd');
	await gotoApp(page, `/maps/${mapId}`);

	await page.getByTestId('palette-trigger').click();
	await page.getByPlaceholder('System, alias, occupier or notes…').fill('Jita');
	const add = page.getByTestId('palette-add').first();
	await expect(add).toContainText('Jita');
	await add.click();

	await expect(page.getByTestId('system-node').filter({ hasText: 'Jita' })).toBeVisible();
});

test('notes match for members and stay hidden from viewers', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E PaletteNotes');
	const added = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: JITA, x: 200, y: 200, alias: null }
	});
	const placementId = (await added.json()).id as number;
	const noted = await api.post(`/api/maps/${mapId}/systems/set-notes`, {
		data: {
			map_id: mapId,
			map_solar_system_id: placementId,
			notes: 'Bookmarked the hauler perch here'
		}
	});
	expect(noted.ok()).toBe(true);

	await gotoApp(page, `/maps/${mapId}`);
	await page.getByTestId('palette-trigger').click();
	await page.getByPlaceholder('System, alias, occupier or notes…').fill('hauler perch');
	await expect(page.getByTestId('palette-hit').first()).toContainText('hauler perch');

	// A viewer never sees notes, so the same query finds nothing on the map.
	const viewer = await createIdentity(4);
	await grantAccess(mapId, viewer.characterId, 'viewer');
	const ctx = await page.context().browser()!.newContext();
	await ctx.addCookies([
		{ name: 'vector_session', value: viewer.session, domain: 'localhost', path: '/' }
	]);
	const viewerPage = await ctx.newPage();
	await viewerPage.goto(`http://localhost:5173/maps/${mapId}`);
	await viewerPage.waitForSelector('html[data-hydrated="true"]');
	await viewerPage.getByTestId('palette-trigger').click();
	await viewerPage.getByPlaceholder('System, alias, occupier or notes…').fill('hauler perch');
	await expect(viewerPage.getByTestId('palette-hit')).toHaveCount(0);
	await ctx.close();
});
