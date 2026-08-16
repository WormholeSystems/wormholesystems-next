import { expect, gotoApp, test } from './fixtures';
import { createIdentity, grantAccess } from './db';

// The redesigned navigation panel: always-visible A→B route, the shared watchlist
// with unified-origin jump counts, per-hop ignore, route settings, and Find.

const JITA = 30000142;
const AMARR = 30002187;
const PERIMETER = 30000144;
const J122515 = 31001882;

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

async function addSystem(
	api: import('@playwright/test').APIRequestContext,
	mapId: number,
	solarSystemId: number,
	x: number,
	y: number
) {
	const res = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: solarSystemId, x, y, alias: null }
	});
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

async function pickSystem(
	page: import('@playwright/test').Page,
	picker: string,
	query: string,
	name: string
) {
	await page.getByTestId(picker).click();
	const input = page.getByPlaceholder('Search…').last();
	await input.fill(query);
	await expect(page.getByRole('option', { name: new RegExp(name) }).last()).toBeVisible();
	await input.press('Enter');
	// Confirm the pick landed before moving on (guards against stale dropdown races).
	await expect(page.getByTestId(picker)).toContainText(name);
}

async function setRoute(page: import('@playwright/test').Page, from: string, to: string) {
	await pickSystem(page, 'system-picker-origin', from.toLowerCase(), from);
	await pickSystem(page, 'system-picker-destination', to.toLowerCase(), to);
}

test('watchlist: add, unified origin badge, pin, remove, reload persistence', async ({
	page,
	api
}) => {
	const mapId = await createMap(api, 'E2E NavWatch');
	await addSystem(api, mapId, JITA, 200, 200);
	await gotoApp(page, `/maps/${mapId}`);

	// Add Amarr via the header plus.
	await page.getByTestId('watchlist-add').click();
	await page.getByPlaceholder('Watch a system…').fill('amarr');
	await page.getByRole('option', { name: /Amarr/ }).first().click();
	const row = page.getByTestId('watchlist-row');
	await expect(row).toHaveCount(1);
	await expect(row.getByText('Amarr')).toBeVisible();
	// No origin yet → no jump count.
	await expect(row.getByText('--')).toBeVisible();

	// Activating Jita on the map becomes the origin: header + 11j badge.
	await page.getByTestId('system-node').filter({ hasText: 'Jita' }).click();
	await expect(page.getByText('· from Jita')).toBeVisible();
	await expect(row.getByText('11j')).toBeVisible();

	// The jump badge opens the hop popover.
	await row.getByTestId('jump-badge').click();
	const popover = page.getByTestId('route-popover');
	await expect(popover).toBeVisible();
	await expect(popover.getByText('11 jumps')).toBeVisible();
	const lastHop = popover.getByTestId('route-list').getByText('Amarr');
	await lastHop.scrollIntoViewIfNeeded();
	await expect(lastHop).toBeVisible();
	await page.keyboard.press('Escape');

	// Pin marks the entry and survives a reload (server-side rows).
	await row.getByLabel('Pin Amarr').click();
	await expect
		.poll(async () => (await (await api.get(`/api/maps/${mapId}/watchlist`)).json())[0].is_pinned)
		.toBe(true);
	await page.reload();
	await page.waitForSelector('html[data-hydrated="true"]');
	await expect(page.getByTestId('watchlist-row')).toHaveCount(1);

	// Remove empties the list.
	await page.getByTestId('watchlist-row').getByLabel('Remove Amarr').click();
	await expect(page.getByTestId('watchlist-row')).toHaveCount(0);
	await expect(page.getByText('Watchlist empty')).toBeVisible();
});

test('route ignore: per-hop X reroutes, Clear restores', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E NavIgnore');
	await gotoApp(page, `/maps/${mapId}`);
	await setRoute(page, 'Jita', 'Amarr');
	await expect(page.getByTestId('route-jumps')).toHaveText('11 jumps');

	// Ignore a middle hop: the route now avoids it and the indicator appears.
	await page.getByTestId('route-list').getByLabel('Ignore Ashab').click();
	await expect(page.getByTestId('route-list').getByText('Ashab')).toHaveCount(0);
	await expect(page.getByTestId('clear-ignored')).toContainText('1 ignored');
	await page.getByTestId('clear-ignored').click();
	await expect(page.getByTestId('clear-ignored')).toHaveCount(0);
	await expect(page.getByTestId('route-jumps')).toHaveText('11 jumps');
});

test('route settings: preference persists; lifetime tolerance drops EOL holes', async ({
	page,
	api
}) => {
	const mapId = await createMap(api, 'E2E NavSettings');
	const a = await addSystem(api, mapId, JITA, 200, 200);
	const b = await addSystem(api, mapId, AMARR, 560, 200);
	const conn = await api.post(`/api/maps/${mapId}/connections/add`, {
		data: { map_id: mapId, from_system: a, to_system: b, kind: 'wormhole' }
	});
	const connId = (await conn.json()).id as number;
	await api.post(`/api/maps/${mapId}/connections/set-status`, {
		data: { map_id: mapId, connection_id: connId, time_status: 'eol' }
	});

	await gotoApp(page, `/maps/${mapId}`);
	await setRoute(page, 'Jita', 'Amarr');
	// Default tolerance allows the EOL hole: one jump.
	await expect(page.getByTestId('route-jumps')).toHaveText('1 jumps');

	// Healthy Only drops the edge → the 11-jump gate route.
	await page.getByTestId('route-settings').click();
	await page.getByTestId('setting-route_allow_time_status').click();
	await page.getByRole('option', { name: 'Healthy Only' }).click();
	await expect(page.getByTestId('route-jumps')).toHaveText('11 jumps');

	// Preference persists server-side across reloads.
	await page.getByTestId('setting-route_preference').click();
	await page.getByRole('option', { name: 'Safer' }).click();
	await page.keyboard.press('Escape');
	await page.reload();
	await page.waitForSelector('html[data-hydrated="true"]');
	await expect(page.getByTestId('navigation-card').getByText('Safer')).toBeVisible();
});

test('find: closest systems from the unified origin', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E NavFind');
	const jita = await addSystem(api, mapId, JITA, 200, 200);
	// A mapped neighbour, so route highlighting has an edge to light up.
	const perimeter = await addSystem(api, mapId, PERIMETER, 560, 200);
	await api.post(`/api/maps/${mapId}/connections/add`, {
		data: { map_id: mapId, from_system: jita, to_system: perimeter, kind: 'wormhole' }
	});
	await gotoApp(page, `/maps/${mapId}`);
	await page.getByTestId('system-node').filter({ hasText: 'Jita' }).click();

	await page.getByTestId('find-toggle').click();
	// Jove observatories: results with jump badges appear.
	await expect(page.getByTestId('find-row').first()).toBeVisible();

	// NPC stations: Jita itself matches at zero jumps.
	await page.getByTestId('find-condition').selectOption('npc_stations');
	await expect(page.getByTestId('find-row').first().getByText('Jita')).toBeVisible();
	await expect(page.getByTestId('find-row').first().getByText('0j')).toBeVisible();

	// Station services (seeded from the SDE) name the concrete stations, collapsed
	// behind a per-row toggle so long lists stay scannable.
	await page.getByTestId('find-condition').selectOption({ label: 'Repair Facilities' });
	await expect(page.getByTestId('find-row').first().getByText('Jita')).toBeVisible();
	await expect(page.getByTestId('find-row').first().getByText('0j')).toBeVisible();
	await expect(page.getByTestId('find-station')).toHaveCount(0);
	// Clicking anywhere on the row expands it (the count chevron is only an indicator).
	await page.getByTestId('find-row').first().click();
	const stations = page.getByTestId('find-station');
	await expect(stations.first()).toBeVisible();
	await expect(stations.first()).toContainText('Jita');
	await expect(page.getByTestId('find-row').first()).toHaveAttribute('aria-expanded', 'true');
	// The jump badge opens the hop popover without collapsing the row.
	await page.getByTestId('find-row').first().getByTestId('jump-badge').click();
	await expect(page.getByTestId('route-popover')).toBeVisible();
	await expect(page.getByTestId('find-row').first()).toHaveAttribute('aria-expanded', 'true');
	await page.keyboard.press('Escape');
	// Clicking the row again collapses it.
	await page.getByTestId('find-row').first().click();
	await expect(page.getByTestId('find-station')).toHaveCount(0);

	// Security Offices only exist on CONCORD lowsec stations (in-game quirk), so
	// highsec Jita is never a match and every result names a CONCORD station.
	await page.getByTestId('find-condition').selectOption({ label: 'Security Office' });
	await expect(page.getByTestId('find-row').first()).toBeVisible();
	await expect(page.getByTestId('find-row').first().getByText('Jita')).toHaveCount(0);
	await page.getByTestId('find-row').first().click();
	await expect(page.getByTestId('find-station').first()).toContainText('CONCORD');

	// Hovering a station keeps its system's route highlighted on the canvas (the
	// station belongs to that system, so the highlight must not drop). Perimeter is
	// one mapped jump away, so its route lights up the connection.
	await page.getByTestId('find-condition').selectOption({ label: 'Repair Facilities' });
	const perimeterRow = page.getByTestId('find-row').filter({ hasText: 'Perimeter' });
	await perimeterRow.hover();
	const onRoute = page.locator('path[data-on-route="true"]');
	await expect(onRoute).toHaveCount(1);
	await perimeterRow.click();
	// Only this row is expanded, so its stations are the visible ones.
	await page.getByTestId('find-station').first().hover();
	await expect(onRoute).toHaveCount(1);
	// Leaving the list entirely clears it again.
	await page.getByTestId('find-condition').hover();
	await expect(onRoute).toHaveCount(0);

	// A station row can be right-clicked to set it as the in-game destination.
	await page.getByTestId('find-station').first().click({ button: 'right' });
	await expect(page.getByTestId('destination-menu')).toBeVisible();
	await expect(page.getByTestId('destination-menu').getByTestId('menu-set-destination')).toBeVisible();
	await page.keyboard.press('Escape');
});

test('quick picks fill the route pickers', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E NavPicks');
	await addSystem(api, mapId, J122515, 200, 200);
	await gotoApp(page, `/maps/${mapId}`);
	await page.getByTestId('system-node').filter({ hasText: 'J122515' }).click();

	const picks = page.getByTestId('quick-picks');
	await expect(picks.getByText('J122515')).toBeVisible();
	await picks.getByText('J122515').click();
	await expect(page.getByTestId('system-picker-origin')).toContainText('J122515');
});

test('viewers see the watchlist read-only', async ({ page, api, browser }) => {
	const mapId = await createMap(api, 'E2E NavViewer');
	await addSystem(api, mapId, JITA, 200, 200);
	const owner = await api.post(`/api/maps/${mapId}/watchlist/add`, {
		data: { map_id: mapId, solar_system_id: AMARR }
	});
	expect(owner.ok()).toBe(true);
	const viewer = await createIdentity(3);
	await grantAccess(mapId, viewer.characterId, 'viewer');

	const ctx = await browser.newContext();
	await ctx.addCookies([
		{ name: 'vector_session', value: viewer.session, domain: 'localhost', path: '/' }
	]);
	const viewerPage = await ctx.newPage();
	await viewerPage.goto(`http://localhost:5173/maps/${mapId}`);
	await viewerPage.waitForSelector('html[data-hydrated="true"]');

	const row = viewerPage.getByTestId('watchlist-row');
	await expect(row).toHaveCount(1);
	await expect(viewerPage.getByTestId('watchlist-add')).toHaveCount(0);
	await expect(row.getByLabel(/^Pin/)).toHaveCount(0);
	await expect(row.getByLabel(/^Remove/)).toHaveCount(0);
	await ctx.close();
	void page;
});
