import { expect, gotoApp, test } from './fixtures';
import { createIdentity, grantAccess } from './db';

// Discord alerts: the settings page that creates them, and the gating that decides who
// may. Whether a message actually reaches Discord is the delivery layer's problem and is
// covered by the Rust tests; this is about the rules being editable and safe.

const JITA = 30000142;
const WEBHOOK = 'https://discord.com/api/webhooks/123456789/abcdefghijklmnop';

type Api = import('@playwright/test').APIRequestContext;

async function createMap(api: Api, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

test('a killmail alert can be created, edited and turned off', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Alerts');
	await gotoApp(page, `/maps/${mapId}/settings/alerts`);

	await expect(page.getByTestId('alerts-empty')).toBeVisible();
	await page.getByTestId('alert-new').click();
	await page.getByTestId('alert-name').fill('Kills at home');
	await page.getByTestId('alert-jumps').fill('3');
	await page.getByTestId('alert-webhook').fill(WEBHOOK);
	await page.getByTestId('alert-save').click();

	const row = page.getByTestId('alert-row');
	await expect(row).toHaveCount(1);
	await expect(row).toContainText('Kills at home');
	await expect(row).toContainText('Anything that dies within 3 jumps');
	// The URL is a key to somebody's channel, so the list names it without handing it over.
	await expect(row).toContainText('discord.com');
	await expect(row).not.toContainText('abcdefghijklmnop');

	// Editing keeps the webhook it already has, rather than demanding it be retyped.
	await page.getByTestId('alert-edit').click();
	await page.getByTestId('alert-name').fill('Kills near home');
	await page.getByTestId('alert-save').click();
	await expect(row).toContainText('Kills near home');
	await expect(row).toContainText('discord.com');

	// Turning it off says so, and says why.
	await row.getByRole('switch').click();
	await expect(page.getByTestId('alert-reason')).toContainText('Turned off by hand');

	// And every one of those is on the record.
	const events = page.getByTestId('alert-events');
	await expect(events).toContainText('created');
	await expect(events).toContainText('updated');
	await expect(events).toContainText('disabled');
});

test('filters narrow a killmail alert to who is involved', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E AlertFilters');
	await gotoApp(page, `/maps/${mapId}/settings/alerts`);

	await page.getByTestId('alert-new').click();
	await page.getByTestId('alert-name').fill('Their losses');
	await page.getByTestId('alert-webhook').fill(WEBHOOK);
	await page.getByTestId('alert-add-filter').click();
	await page.getByTestId('alert-filter-ids').fill('99000001, 99000002');
	await page.getByTestId('alert-save').click();

	await expect(page.getByTestId('alert-row')).toContainText('1 filter (any)');

	// The rule survives a round trip through the server.
	const alerts = await (await api.get(`/api/maps/${mapId}/alerts`)).json();
	expect(alerts[0].filters).toEqual([
		{ subject: 'alliance', side: 'either', mode: 'include', ids: [99000001, 99000002] }
	]);
});

test('a proximity alert needs a system, and reports it', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E AlertProximity');
	await gotoApp(page, `/maps/${mapId}/settings/alerts`);

	await page.getByTestId('alert-new').click();
	await page.getByTestId('alert-name').fill('Jita gets close');
	await page.getByTestId('alert-webhook').fill(WEBHOOK);
	await page.getByTestId('alert-kind').click();
	await page.getByRole('option', { name: 'System near the chain' }).click();

	// Without a target there is nothing to measure to, so saving is refused.
	await expect(page.getByTestId('alert-save')).toBeDisabled();

	await page.getByTestId('system-picker-pick a system').click();
	await page.getByPlaceholder('Search…').fill('jita');
	await page.getByTestId('picker-result').first().click();
	await page.getByTestId('alert-save').click();

	await expect(page.getByTestId('alert-row')).toContainText('Jita within 5 jumps');

	const alerts = await (await api.get(`/api/maps/${mapId}/alerts`)).json();
	expect(alerts[0].target_solar_system_id).toBe(JITA);
});

test('members cannot see or change a map alerts', async ({ page, api, browser }) => {
	const mapId = await createMap(api, 'E2E AlertGating');
	const member = await createIdentity(37);
	await grantAccess(mapId, member.characterId, 'member');

	const ctx = await browser.newContext();
	await ctx.addCookies([
		{ name: 'vector_session', value: member.session, domain: 'localhost', path: '/' }
	]);
	const memberPage = await ctx.newPage();
	await memberPage.goto(`http://localhost:5173/maps/${mapId}/settings/alerts`);
	await memberPage.waitForSelector('html[data-hydrated="true"]');
	await expect(memberPage.getByTestId('alerts-error')).toBeVisible();
	await expect(memberPage.getByTestId('alert-new')).toHaveCount(0);
	await ctx.close();
	void page;
});
