import { expect, gotoApp, test } from './fixtures';
import { E2E_CHARACTER_ID, createIdentity, grantAccess, setCharacterPresence } from './db';

// The setup guide on a new map: it says what is still missing, does each thing itself, and
// ticks items off from the map's real state rather than from a step counter.

const J122515 = 31001882;

type Api = import('@playwright/test').APIRequestContext;

async function createMap(api: Api, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

test('a fresh map offers to start where the pilot already is', async ({ page, api }) => {
	await setCharacterPresence(E2E_CHARACTER_ID, J122515);
	const mapId = await createMap(api, 'E2E Setup Start');

	await gotoApp(page, `/maps/${mapId}`);
	const guide = page.getByTestId('setup-guide');
	await expect(guide).toBeVisible();

	// Nothing is done on a map with nothing on it.
	await expect(guide.locator('[data-step="home"]')).toHaveAttribute('data-done', 'false');

	// It knows where the pilot is, so placing the first system is one click.
	const home = guide.locator('[data-step="home"]');
	await expect(home).toContainText('J122515');
	await home.getByTestId('setup-action').click();

	await expect(page.getByTestId('system-node').filter({ hasText: 'J122515' })).toBeVisible();
	// The step ticks itself off from the map, not from having pressed the button.
	await expect(guide.locator('[data-step="home"]')).toHaveAttribute('data-done', 'true', {
		timeout: 10_000
	});

	await setCharacterPresence(E2E_CHARACTER_ID, J122515, false);
});

test('consent is asked for once and takes effect immediately', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Setup Consent');

	await gotoApp(page, `/maps/${mapId}`);
	const step = page.getByTestId('setup-guide').locator('[data-step="tracking"]');
	await expect(step).toHaveAttribute('data-done', 'false');

	await step.getByTestId('setup-action').click();
	await expect(step).toHaveAttribute('data-done', 'true');

	// It is stored, so it holds across a reload and the status bar agrees.
	await page.reload();
	await page.waitForSelector('[data-testid="panel-grid"]');
	await expect(page.getByTestId('tracking-toggle')).toHaveAttribute('aria-pressed', 'true');
});

test('dismissing it sticks, and the status bar brings it back', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Setup Dismiss');

	await gotoApp(page, `/maps/${mapId}`);
	await expect(page.getByTestId('setup-guide')).toBeVisible();

	await page.getByTestId('setup-dismiss').click();
	await expect(page.getByTestId('setup-guide')).toHaveCount(0);

	// Wait for the write, not just the card going away: hiding it is local and instant, so
	// reloading straight after would abort the request that makes it stick.
	await expect
		.poll(
			async () =>
				(await (await api.get(`/api/maps/${mapId}/settings/user`)).json()).setup_dismissed,
			{ timeout: 10_000 }
		)
		.toBe(true);

	// Unlike legacy's wizard, waving it away is remembered rather than being a per-visit
	// annoyance, and unlike legacy it can be brought back at all.
	await page.reload();
	await page.waitForSelector('[data-testid="panel-grid"]');
	await expect(page.getByTestId('setup-guide')).toHaveCount(0);

	await page.getByTestId('setup-toggle').click();
	await expect(page.getByTestId('setup-guide')).toBeVisible();
});

test('a map that is already set up does not nag', async ({ page, api, browser }) => {
	const mapId = await createMap(api, 'E2E Setup Done');
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J122515, x: 200, y: 200, alias: null }
	});
	await api.post(`/api/maps/${mapId}/settings/user`, { data: { tracking_allowed: true } });
	const mate = await createIdentity(41);
	await grantAccess(mapId, mate.characterId, 'member');

	await gotoApp(page, `/maps/${mapId}`);
	await page.waitForSelector('[data-testid="panel-grid"]');
	// Every item is satisfied by the map's own state, so there is nothing to show.
	await expect(page.getByTestId('setup-guide')).toHaveCount(0);

	// It is still reachable, and reports itself complete.
	await page.getByTestId('setup-toggle').click();
	const guide = page.getByTestId('setup-guide');
	await expect(guide).toBeVisible();
	await expect(guide).toContainText('3/3');
	await browser.contexts();
});
