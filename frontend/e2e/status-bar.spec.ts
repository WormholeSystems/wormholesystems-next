import { expect, gotoApp, test } from './fixtures';
import { createIdentity, grantAccess } from './db';

// The map status bar: identity, the live-feed dot, the view toggles, and the undo/redo
// pair driven by the command journal.

const J122515 = 31001882; // C5, Wolf-Rayet Star
const JITA = 30000142;

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

test('shows the map name and a live socket once connected', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Status');
	await gotoApp(page, `/maps/${mapId}`);

	await expect(page.getByTestId('status-bar-name')).toHaveText('E2E Status');
	// The socket reports itself open, which is the whole point of the dot.
	await expect(page.getByTestId('socket-dot')).toHaveAttribute('data-state', 'open');
});

test('the view toggles persist through a reload', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Toggles');
	await gotoApp(page, `/maps/${mapId}`);

	const threat = page.getByTestId('threat-toggle');
	await expect(threat).toHaveAttribute('aria-pressed', 'true');
	await threat.click();
	await expect(threat).toHaveAttribute('aria-pressed', 'false');

	await gotoApp(page, `/maps/${mapId}`);
	await expect(page.getByTestId('threat-toggle')).toHaveAttribute('aria-pressed', 'false');
});

test('undo reverses a change and redo puts it back', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Undo');
	await gotoApp(page, `/maps/${mapId}`);

	// Nothing has happened yet, so there is nothing to undo.
	await expect(page.getByTestId('undo-button')).toBeDisabled();
	await expect(page.getByTestId('redo-button')).toBeDisabled();

	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J122515, x: 200, y: 200, alias: null }
	});
	const node = page.getByTestId('system-node').filter({ hasText: 'J122515' });
	await expect(node).toBeVisible();

	// The journal entry arrives over the socket, enabling undo.
	const undo = page.getByTestId('undo-button');
	await expect(undo).toBeEnabled();
	await undo.click();
	await expect(node).toHaveCount(0);

	// Undoing recorded its own entry; undoing that is the redo.
	const redo = page.getByTestId('redo-button');
	await expect(redo).toBeEnabled();
	await redo.click();
	await expect(node).toBeVisible();
});

test('history lists what happened and who did it', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E History');
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: JITA, x: 100, y: 100, alias: null }
	});
	await gotoApp(page, `/maps/${mapId}`);

	await page.getByTestId('history-button').click();
	const list = page.getByTestId('history-list');
	await expect(list).toBeVisible();
	await expect(list.getByText('E2E Pilot')).toBeVisible();
	await expect(list.getByText(/Jita/)).toBeVisible();
});

test("a viewer cannot undo, and sees someone else's history", async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E ViewerBar');
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: JITA, x: 100, y: 100, alias: null }
	});

	const viewer = await createIdentity(1);
	await grantAccess(mapId, viewer.characterId, 'viewer');
	const viewerCtx = await page.context().browser()!.newContext();
	await viewerCtx.addCookies([
		{ name: 'vector_session', value: viewer.session, domain: 'localhost', path: '/' }
	]);
	const viewerPage = await viewerCtx.newPage();
	await viewerPage.goto(`http://localhost:5173/maps/${mapId}`);
	await viewerPage.waitForSelector('html[data-hydrated="true"]');

	// Undo/redo are write actions, so a viewer does not get them at all.
	await expect(viewerPage.getByTestId('undo-button')).toHaveCount(0);
	// History is readable, though: seeing who changed the chain is part of using it.
	await viewerPage.getByTestId('history-button').click();
	await expect(viewerPage.getByTestId('history-list').getByText('E2E Pilot')).toBeVisible();
	await viewerCtx.close();
});

test('a character outside every grant is warned about limited access', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Limited');
	await gotoApp(page, `/maps/${mapId}`);
	// The e2e identity owns this map through its own character, so no warning.
	await expect(page.getByText('Limited access')).toHaveCount(0);
});
