import { expect, gotoApp, test } from './fixtures';
import { createIdentity, grantAccess } from './db';

// Map settings: renaming, and the access list that decides who sees the chain.

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

test('renaming the map shows up on the map itself', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Before');
	await gotoApp(page, `/maps/${mapId}/settings`);

	const input = page.getByTestId('map-name-input');
	await expect(input).toHaveValue('E2E Before');
	await input.fill('E2E After');
	await page.getByTestId('rename-button').click();
	// Save goes disabled once the reloaded map carries the new name, which is the signal
	// that the write landed. Re-checking the input would pass instantly, since `fill` had
	// already set it, and navigating then aborts the request in flight.
	await expect(page.getByTestId('rename-button')).toBeDisabled();

	await gotoApp(page, `/maps/${mapId}`);
	await expect(page.getByTestId('status-bar-name')).toHaveText('E2E After');
});

test('the owner is listed, and granting adds a second entry', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Access');
	const mate = await createIdentity(2);
	await gotoApp(page, `/maps/${mapId}/settings`);

	const list = page.getByTestId('access-list');
	await expect(list.getByText('E2E Pilot')).toBeVisible();

	// The grant search only knows entities Vector has cached, so a known character resolves.
	await page.getByTestId('grant-search').fill(String(mate.characterId));
	await page.getByTestId('grant-button').click();
	await expect(list.getByRole('listitem')).toHaveCount(2);
});

test('a viewer sees the roles but cannot change them', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E ViewerSettings');
	const viewer = await createIdentity(3);
	await grantAccess(mapId, viewer.characterId, 'viewer');

	const ctx = await page.context().browser()!.newContext();
	await ctx.addCookies([
		{ name: 'vector_session', value: viewer.session, domain: 'localhost', path: '/' }
	]);
	const viewerPage = await ctx.newPage();
	await viewerPage.goto(`http://localhost:5173/maps/${mapId}/settings`);
	await viewerPage.waitForSelector('html[data-hydrated="true"]');

	await expect(viewerPage.getByTestId('access-list').getByText('E2E Pilot')).toBeVisible();
	// No grant form, no delete card: both are manager/owner work.
	await expect(viewerPage.getByTestId('grant-search')).toHaveCount(0);
	await expect(viewerPage.getByTestId('delete-map')).toHaveCount(0);
	await expect(viewerPage.getByTestId('map-name-input')).toBeDisabled();
	await ctx.close();
});
