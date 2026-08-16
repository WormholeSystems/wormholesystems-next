import { test as base } from '@playwright/test';

import { E2E_SESSION } from './db';

// All tests run as the e2e identity created in global-setup: the session cookie is set on
// the browser context, and `api` is a request context carrying the same cookie for
// arranging state directly against the backend.
export const test = base.extend<{ api: import('@playwright/test').APIRequestContext }>({
	context: async ({ context }, use) => {
		await context.addCookies([
			{ name: 'vector_session', value: E2E_SESSION, domain: 'localhost', path: '/' }
		]);
		await use(context);
	},
	api: async ({ playwright }, use) => {
		const api = await playwright.request.newContext({
			baseURL: 'http://127.0.0.1:3000',
			extraHTTPHeaders: { cookie: `vector_session=${E2E_SESSION}` }
		});
		await use(api);
		await api.dispose();
	}
});

export { expect } from '@playwright/test';

/** Navigate and wait until the page is hydrated (controls are interactive). */
export async function gotoApp(page: import('@playwright/test').Page, path: string) {
	await page.goto(path);
	await page.waitForSelector('html[data-hydrated="true"]');
}
