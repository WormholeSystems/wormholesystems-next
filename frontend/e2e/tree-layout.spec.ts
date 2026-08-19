import { expect, gotoApp, test } from './fixtures';

// Automatic placement: the chain drawn as a tree from its connections, instead of from
// wherever people dragged the systems.

const J122515 = 31001882;
const JITA = 30000142;
const PERIMETER = 30000144;

async function createMap(api: import('@playwright/test').APIRequestContext, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

async function addSystem(
	api: import('@playwright/test').APIRequestContext,
	mapId: number,
	solarSystemId: number,
	x: number,
	y: number
) {
	const res = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: solarSystemId, x, y, alias: null }
	});
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

async function connect(
	api: import('@playwright/test').APIRequestContext,
	mapId: number,
	from: number,
	to: number
) {
	const res = await api.post(`/api/maps/${mapId}/connections/add`, {
		data: { map_id: mapId, from_system: from, to_system: to, kind: 'wormhole' }
	});
	expect(res.ok()).toBe(true);
}

/** A chain of three, deliberately placed in a heap so the layout has work to do. */
async function seedChain(api: import('@playwright/test').APIRequestContext, name: string) {
	const mapId = await createMap(api, name);
	const home = await addSystem(api, mapId, J122515, 900, 700);
	const second = await addSystem(api, mapId, JITA, 900, 800);
	const third = await addSystem(api, mapId, PERIMETER, 900, 900);
	await connect(api, mapId, home, second);
	await connect(api, mapId, home, third);
	await api.post(`/api/maps/${mapId}/systems/set-home`, {
		data: { map_id: mapId, map_solar_system_id: home, value: true }
	});
	return { mapId, home, second, third };
}

function nodePosition(page: import('@playwright/test').Page, text: string) {
	return page
		.getByTestId('system-node')
		.filter({ hasText: text })
		.evaluate((el) => ({
			left: parseFloat((el as HTMLElement).style.left),
			top: parseFloat((el as HTMLElement).style.top)
		}));
}

test('a map set to automatic placement draws the chain as a tree', async ({ page, api }) => {
	const { mapId } = await seedChain(api, 'E2E Tree');
	await api.post(`/api/maps/${mapId}/update`, {
		data: { map_id: mapId, layout: 'tree' }
	});

	await gotoApp(page, `/maps/${mapId}`);
	await expect(page.getByTestId('system-node')).toHaveCount(3);

	// The home system roots the tree in the first column; its two holes share the next.
	const home = await nodePosition(page, 'J122515');
	const jita = await nodePosition(page, 'Jita');
	const perimeter = await nodePosition(page, 'Perimeter');
	expect(home.left).toBe(60);
	expect(jita.left).toBe(380);
	expect(perimeter.left).toBe(380);
	expect(jita.top).not.toBe(perimeter.top);

	// Nothing to drag: the layout owns the positions.
	await expect(page.getByTestId('drag-handle')).toHaveCount(0);
});

test('a viewer can pick their own placement when the map allows it', async ({ page, api }) => {
	const { mapId } = await seedChain(api, 'E2E TreeOverride');
	await api.post(`/api/maps/${mapId}/update`, {
		data: { map_id: mapId, allow_layout_override: true }
	});

	await gotoApp(page, `/maps/${mapId}`);
	await expect(page.getByTestId('system-node')).toHaveCount(3);
	// The map is still on custom placement, so the systems sit where they were put.
	expect((await nodePosition(page, 'Jita')).left).toBe(900);

	await page.getByTestId('placement-tree').click();
	await expect.poll(async () => (await nodePosition(page, 'Jita')).left).toBe(380);

	// It is a preference of this viewer's, so it survives a reload. The switch paints
	// before the write lands, so wait for the stored value: reloading first would cancel
	// the request in flight.
	await expect
		.poll(async () => (await (await api.get(`/api/maps/${mapId}/settings/user`)).json()).layout_override)
		.toBe('tree');
	await page.reload();
	await page.waitForSelector('html[data-hydrated="true"]');
	await expect(page.getByTestId('system-node')).toHaveCount(3);
	await expect.poll(async () => (await nodePosition(page, 'Jita')).left).toBe(380);

	// And going back to the map's own mode restores the dragged positions.
	await page.getByTestId('placement-manual').click();
	await expect.poll(async () => (await nodePosition(page, 'Jita')).left).toBe(900);
});

test('the switcher is hidden unless the map hands the choice over', async ({ page, api }) => {
	const { mapId } = await seedChain(api, 'E2E TreeNoOverride');
	await gotoApp(page, `/maps/${mapId}`);
	await expect(page.getByTestId('placement-controls')).toHaveCount(0);
});

test('the creator picks the placement, with both options explained', async ({ page, api }) => {
	await gotoApp(page, '/maps');
	await page.getByTestId('new-map').click();
	await page.getByTestId('new-map-name').fill('E2E TreeCreated');

	// Both options say what they do, so the choice can be made without leaving the dialog.
	await expect(page.getByTestId('new-map-layout-manual')).toContainText('drag the systems');
	await expect(page.getByTestId('new-map-layout-tree')).toContainText(
		'draws itself from the connections'
	);
	// Custom placement is the default.
	await expect(page.getByTestId('new-map-layout-manual')).toHaveAttribute('aria-pressed', 'true');

	await page.getByTestId('new-map-layout-tree').click();
	await page.getByTestId('new-map-create').click();
	await page.waitForURL(/\/maps\/\d+/);
	await page.waitForSelector('[data-testid="panel-grid"]');

	const mapId = Number(page.url().match(/\/maps\/(\d+)/)![1]);
	const view = await (await api.get(`/api/maps/${mapId}`)).json();
	expect(view.map.layout).toBe('tree');

	await api.delete(`/api/maps/${mapId}`);
});
