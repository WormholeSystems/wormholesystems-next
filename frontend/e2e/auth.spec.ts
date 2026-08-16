import { expect, test } from '@playwright/test';

// Anonymous tests: no session cookie (uses the plain Playwright test, not the fixture).

test('home page renders', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Vector' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Open your maps' })).toBeVisible();
});

test('maps requires login', async ({ page }) => {
	await page.goto('/maps');
	await expect(page).toHaveURL('/login');
	await expect(page.getByRole('link', { name: 'Log in with EVE Online' })).toBeVisible();
});
