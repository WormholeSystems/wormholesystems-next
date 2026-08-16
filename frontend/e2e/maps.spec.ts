import { expect, gotoApp, test } from './fixtures';

test('nav shows the signed-in character', async ({ page }) => {
	await gotoApp(page, '/');
	await page.getByLabel('Account').click();
	await expect(page.getByText('E2E Pilot')).toBeVisible();
	await expect(page.getByText('Log out')).toBeVisible();
});

test('create and delete a map', async ({ page }) => {
	await gotoApp(page, '/maps');
	await page.getByPlaceholder('New map name').fill('E2E List Map');
	await page.getByRole('button', { name: 'Create' }).click();

	const row = page.getByRole('listitem').filter({ hasText: 'E2E List Map' });
	await expect(row).toBeVisible();
	await expect(row.getByText('OWNER')).toBeVisible();

	await row.hover();
	await row.getByRole('button', { name: 'Delete' }).click();
	await expect(row).toHaveCount(0);
});
