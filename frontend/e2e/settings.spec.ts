import { expect, gotoApp, test } from './fixtures';
import { createIdentity, grantAccess } from './db';

// Map settings: renaming on the General section, the names the map hands out under
// Naming, and the Access section that decides who sees the chain.

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
	await gotoApp(page, `/maps/${mapId}/settings/access`);

	const list = page.getByTestId('access-list');
	await expect(list.getByText('E2E Pilot')).toBeVisible();

	// The grant search only knows entities Vector has cached, so a known character resolves.
	await page.getByTestId('grant-search').fill(String(mate.characterId));
	await page.getByTestId('grant-button').click();
	await expect(list.getByRole('listitem')).toHaveCount(2);
});

test('a grant can be given an end date, and taken back to permanent', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Expiry');
	const mate = await createIdentity(4);
	await gotoApp(page, `/maps/${mapId}/settings/access`);

	await page.getByTestId('grant-search').fill(String(mate.characterId));
	await page.getByTestId('grant-duration').click();
	await page.getByRole('option', { name: 'For a day' }).click();
	await page.getByTestId('grant-button').click();

	const list = page.getByTestId('access-list');
	await expect(list.getByRole('listitem')).toHaveCount(2);
	// The row says when it runs out, rather than looking like any other grant.
	await expect(list.getByTestId('access-expiry')).toHaveCount(1);
	const stored = await (await api.get(`/api/maps/${mapId}/access`)).json();
	expect(stored.find((e: { subject_id: number }) => e.subject_id === mate.characterId).expires_at)
		.not.toBeNull();

	// Clicking it drops the end date.
	await list.getByTestId('access-expiry').click();
	await expect(list.getByTestId('access-expiry')).toHaveCount(0);
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
	await viewerPage.goto(`http://localhost:5173/maps/${mapId}/settings/access`);
	await viewerPage.waitForSelector('html[data-hydrated="true"]');

	await expect(viewerPage.getByTestId('access-list').getByText('E2E Pilot')).toBeVisible();
	// No grant form: granting is manager/owner work.
	await expect(viewerPage.getByTestId('grant-search')).toHaveCount(0);
	// And the sections that change the map for everyone are not offered at all, rather
	// than offered and refused.
	const nav = viewerPage.getByTestId('settings-nav');
	await expect(nav).toContainText('Routing');
	await expect(nav).not.toContainText('General');
	await expect(nav).not.toContainText('Discord alerts');
	await ctx.close();
});

test('chain naming previews as you type and survives a reload', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Naming');
	await gotoApp(page, `/maps/${mapId}/settings/naming`);

	// The defaults are legacy's, so an untouched map already names things sensibly.
	await expect(page.getByTestId('alias-preview')).toHaveText('1, 2, 3, 11');
	await expect(page.getByTestId('bookmark_wormhole-preview')).toHaveText('1a ABC C5');
	await expect(page.getByTestId('bookmark_kspace-preview')).toHaveText('1b HS ABC Jita The Forge');

	// Alphabetical skips H, L, N and P, which belong to the k-space exits.
	await page.getByTestId('alias-scheme').getByText('Alphabetical').click();
	await expect(page.getByTestId('alias-preview')).toHaveText('A, B, C, AA');

	await page.getByTestId('bookmark_wormhole').fill('{alias} {sig} {class} {wh} {life}');
	await expect(page.getByTestId('bookmark_wormhole-preview')).toHaveText('1a ABC C5 H296 EOL');

	await page.getByTestId('save-naming').click();
	await expect(page.getByTestId('save-naming')).toBeDisabled();

	await page.reload();
	await expect(page.getByTestId('bookmark_wormhole')).toHaveValue(
		'{alias} {sig} {class} {wh} {life}'
	);
	await expect(page.getByTestId('alias-preview')).toHaveText('A, B, C, AA');

	// Copying the bookmark is per user, and gated on location sharing a fresh map lacks.
	const copy = page.locator('[data-setting="copy-bookmark"]');
	await expect(copy.getByRole('switch')).toBeDisabled();
	await expect(copy).toContainText('Needs location sharing');
});

test('settings are split into sections, and the per-user ones save themselves', async ({
	page,
	api
}) => {
	const mapId = await createMap(api, 'E2E Sections');
	await gotoApp(page, `/maps/${mapId}/settings`);

	// Every section is reachable from any other.
	const nav = page.getByTestId('settings-nav');
	await expect(nav.getByTestId('settings-section')).toHaveCount(7);
	await expect(nav.locator('[data-active="true"]')).toContainText('General');

	// A per-user setting saves on the spot: there is no form to submit.
	await nav.getByText('Routing').click();
	await expect(page.getByTestId('route-preference')).toBeVisible();
	await page.getByTestId('route-preference').click();
	await page.getByRole('option', { name: 'Safer' }).click();
	await expect
		.poll(async () => (await (await api.get(`/api/maps/${mapId}/settings/user`)).json()).route_preference)
		.toBe('safer');

	// And it survives leaving and coming back, because it is stored per map, not per tab.
	await nav.getByText('Display').click();
	await nav.getByText('Routing').click();
	await expect(page.getByTestId('route-preference')).toContainText('Safer');
});

test('mapping settings hold back what location sharing gates', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Gated');
	await gotoApp(page, `/maps/${mapId}/settings/mapping`);

	// Sharing is off on a new map, so the ones that depend on it say why rather than
	// silently doing nothing.
	const prompt = page.locator('[data-setting="prompt-for-signature"]');
	await expect(prompt.getByRole('switch')).toBeDisabled();
	await expect(prompt).toContainText('Needs location sharing');
});
