import { expect, gotoApp, test } from './fixtures';

// Account linking. The OAuth round trip needs a real Discord app, so what is testable here
// is the page's two states and the API behind them.

test('the settings page offers to connect when nothing is linked', async ({ page }) => {
	await gotoApp(page, '/settings/discord');
	await expect(page.getByTestId('discord-unlinked')).toBeVisible();
	await expect(page.getByTestId('discord-connect')).toHaveAttribute('href', '/discord/connect');
	// And it says what the bot can do, so the page is worth visiting before linking.
	await expect(page.getByText('/vector route')).toBeVisible();
});

test('connecting without a Discord app configured says so rather than 500ing', async ({ api }) => {
	const res = await api.get('/discord/connect', { maxRedirects: 0 });
	// 404 when unconfigured, 302 to Discord when it is; either way, never a server error.
	expect([302, 303, 404]).toContain(res.status());
});

test('an unsigned interaction is rejected', async ({ api }) => {
	const res = await api.post('/discord/interactions', { data: { type: 1 } });
	expect([401, 404]).toContain(res.status());
});
