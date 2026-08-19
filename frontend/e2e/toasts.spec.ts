import { expect, gotoApp, test } from './fixtures';

// Transient feedback. Anything a person did that the map does not show by itself says so
// in a toast; a change you can see on the map still says nothing.

const J122515 = 31001882;

test('a failed action says why, and a silent one confirms itself', async ({ page, api }) => {
	const res = await api.post('/api/maps', { data: { name: 'E2E Toasts' } });
	const mapId = (await res.json()).id as number;
	await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J122515, x: 300, y: 300, alias: null }
	});

	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);

	// A paste with nothing in it fails in the panel, which has nothing to show for it.
	await expect(page.getByTestId('signatures-card')).toBeVisible();
	await page.evaluate(() => {
		const dt = new DataTransfer();
		dt.setData('text/plain', 'not a scan at all');
		window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt }));
	});
	await expect(page.getByText('Nothing in that paste looked like a signature')).toBeVisible();

	// Copying the layout goes to the clipboard, where nothing on screen would show it.
	await page.context().grantPermissions(['clipboard-write']);
	await page.getByTestId('layout-toggle').click();
	await page.getByTestId('layout-more').click();
	await page.getByTestId('layout-copy').click();
	await expect(page.getByText('Layout copied')).toBeVisible();

	await api.delete(`/api/maps/${mapId}`);
});
