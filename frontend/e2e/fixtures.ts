import { test as base } from '@playwright/test';

import { E2E_SESSION } from './db';

// All tests run as the e2e identity created in global-setup: the session cookie is set on
// the browser context, and `api` is a request context carrying the same cookie for
// arranging state directly against the backend.
export const test = base.extend<{ api: import('@playwright/test').APIRequestContext }>({
	context: async ({ context }, use) => {
		await context.addCookies([
			{ name: 'ws_session', value: E2E_SESSION, domain: 'localhost', path: '/' }
		]);
		await use(context);
	},
	api: async ({ playwright }, use) => {
		const api = await playwright.request.newContext({
			baseURL: 'http://127.0.0.1:3000',
			extraHTTPHeaders: { cookie: `ws_session=${E2E_SESSION}` }
		});
		await use(api);
		await api.dispose();
	}
});

export { expect } from '@playwright/test';

/**
 * Navigate and wait until the page is hydrated (controls are interactive).
 *
 * A map you have not been through the introduction on opens it as a modal over everything,
 * so this marks it done first. Pass `{ introduction: true }` to leave it alone, which is
 * what the introduction's own spec does.
 */
export async function gotoApp(
	page: import('@playwright/test').Page,
	path: string,
	options: { introduction?: boolean } = {}
) {
	const map = /\/maps\/(\d+)(\?|$)/.exec(path);
	if (map && !options.introduction) {
		// The API is a different host from the app, so the context cookie is not sent with it.
		await page.request.post(`http://127.0.0.1:3000/api/maps/${map[1]}/settings/user`, {
			headers: { cookie: `ws_session=${E2E_SESSION}` },
			data: { introduction_confirmed: true }
		});
	}
	await page.goto(path);
	await page.waitForSelector('html[data-hydrated="true"]');
	// A map page holds a loader until its graph and panel arrangement have both arrived,
	// so hydration alone is not enough to start clicking.
	if (/\/maps\/\d+(\?|$)/.test(path)) {
		await page.waitForSelector('[data-testid="panel-grid"]');
	}
}
