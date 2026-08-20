import { expect, gotoApp, test } from './fixtures';

test('shot', async ({ page }) => {
	await page.setViewportSize({ width: 1440, height: 1000 });
	await gotoApp(page, '/');
	await page.getByTestId('landing-stat').first().waitFor();
	await page.evaluate(() => document.documentElement.classList.add('dark'));
	for (let y = 0; y < 7000; y += 400) {
		await page.evaluate((to) => window.scrollTo(0, to), y);
		await page.waitForTimeout(60);
	}
	await page.evaluate(() => window.scrollTo(0, 0));
	await page.waitForTimeout(900);
	await page.screenshot({ path: 'shot-v6.png', fullPage: true, animations: 'disabled' });
	expect(1).toBe(1);
});
