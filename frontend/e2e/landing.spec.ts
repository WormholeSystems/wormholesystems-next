import { expect, gotoApp, test } from './fixtures';

// The landing page: what someone sees before they have any reason to trust this.

test('the landing page states what it is and how to run it', async ({ page }) => {
	await gotoApp(page, '/');

	await expect(page.getByRole('heading', { level: 1 })).toContainText('Map the chain');
	await expect(page.getByTestId('landing-feature')).toHaveCount(6);

	// The install commands are the point of the self-host section, so they are real text on
	// the page rather than an image someone has to retype from.
	await expect(page.getByText('./vectorctl setup')).toBeVisible();

	await page.getByRole('link', { name: 'Open your maps' }).click();
	await expect(page).toHaveURL(/\/maps$/);
});

test('the page does not scroll sideways on a phone', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await gotoApp(page, '/');
	await page.getByTestId('landing-feature').first().waitFor();

	const overflows = await page.evaluate(
		() => document.documentElement.scrollWidth > document.documentElement.clientWidth
	);
	expect(overflows).toBe(false);
});
