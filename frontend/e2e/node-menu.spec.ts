import { expect, gotoApp, test } from './fixtures';

// Legacy context-menu parity and interaction polish.

const JITA = 30000142; // highsec k-space
const J171828 = 31002580; // C13 (frigate shattered)
const J122515 = 31001882; // C5 wormhole

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

test('menu structure: submenus, external links, remove gating', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Menu');
	const jita = await addSystem(api, mapId, JITA, 200, 200);

	await gotoApp(page, `/maps/${mapId}`);
	const node = page.getByTestId('system-node').filter({ hasText: 'Jita' });
	await node.click({ button: 'right' });
	const menu = page.getByTestId('context-menu');

	// Order and presence.
	await expect(menu.getByRole('button', { name: 'Pin', exact: true })).toBeVisible();
	await expect(menu.getByTestId('status-subtrigger')).toBeVisible();
	await expect(menu.getByTestId('external-subtrigger')).toBeVisible();
	await expect(menu.getByRole('button', { name: 'Set as Home System' })).toBeVisible();
	await expect(menu.getByRole('button', { name: 'Set as Rally Point' })).toBeVisible();
	await expect(menu.getByRole('button', { name: 'Remove' })).toBeVisible();

	// Status submenu opens on hover with icon entries.
	await menu.getByTestId('status-subtrigger').hover();
	await expect(menu.getByTestId('status-submenu').getByRole('button', { name: 'Hostile' })).toBeVisible();

	// External submenu: k-space gets Dotlan Jump Range and correct zKillboard links.
	await menu.getByTestId('external-subtrigger').hover();
	const ext = menu.getByTestId('external-submenu');
	await expect(ext.getByRole('link', { name: 'Jump Range' })).toHaveAttribute(
		'href',
		'https://evemaps.dotlan.net/range/Revelation,5/Jita'
	);
	await expect(ext.getByRole('link', { name: 'System' }).last()).toHaveAttribute(
		'href',
		`https://zkillboard.com/system/${JITA}/`
	);
	await expect(ext.getByRole('link', { name: 'Region', exact: true })).toHaveAttribute(
		'href',
		'https://zkillboard.com/region/10000002/'
	);

	// Pin the system: Remove disappears (pinned systems are protected).
	await menu.getByRole('button', { name: 'Pin', exact: true }).click();
	await expect(node.getByTestId('drag-handle')).toHaveCount(0);
	await node.click({ button: 'right' });
	await expect(menu.getByRole('button', { name: 'Unpin' })).toBeVisible();
	await expect(menu.getByRole('button', { name: 'Remove' })).toHaveCount(0);
	await page.keyboard.press('Escape');

	// A wormhole node's External submenu has no Jump Range.
	await api.post(`/api/maps/${mapId}/systems/set-pinned`, {
		data: { map_id: mapId, map_solar_system_id: jita, value: false }
	});
	await addSystem(api, mapId, J122515, 200, 400);
	const wh = page.getByTestId('system-node').filter({ hasText: 'J122515' });
	await wh.click({ button: 'right' });
	await menu.getByTestId('external-subtrigger').hover();
	await expect(menu.getByTestId('external-submenu').getByRole('link', { name: 'Jump Range' })).toHaveCount(0);
});

test('marquee selects live and remove hijacks the selection', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Marquee');
	await addSystem(api, mapId, JITA, 200, 200);
	await addSystem(api, mapId, J122515, 200, 320);

	await gotoApp(page, `/maps/${mapId}`);
	const canvas = page.getByTestId('map-canvas');
	const box = (await canvas.boundingBox())!;

	// Drag a marquee over both nodes: selection updates live during the drag.
	await page.mouse.move(box.x + 150, box.y + 150);
	await page.mouse.down();
	await page.mouse.move(box.x + 450, box.y + 400, { steps: 8 });
	const nodes = page.getByTestId('system-node');
	await expect(nodes.first()).toHaveClass(/bg-amber/);
	await page.mouse.up();

	// Selection is sticky after release.
	await expect(nodes.first()).toHaveClass(/bg-amber/);
	await expect(nodes.last()).toHaveClass(/bg-amber/);

	// Remove on one node deletes the whole selection.
	await nodes.first().click({ button: 'right' });
	await page.getByTestId('context-menu').getByRole('button', { name: 'Remove' }).click();
	await expect(nodes).toHaveCount(0);
});

test('connection from a C13 system defaults to frigate size', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E ShipSize');
	// Find a C13 via search is unreliable; use the API map view to assert the size flows.
	const from = await addSystem(api, mapId, JITA, 200, 200);
	const c13 = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: J171828, x: 200, y: 400, alias: null }
	});
	expect(c13.ok()).toBe(true);
	const c13id = (await c13.json()).id as number;

	await gotoApp(page, `/maps/${mapId}`);
	// Drag from Jita's connection handle onto the C13 node.
	const jita = page.getByTestId('system-node').filter({ hasText: 'Jita' });
	const target = page.getByTestId('system-node').filter({ hasText: 'J' }).last();
	await jita.hover();
	const handle = jita.getByTestId('connection-handle');
	const hb = (await handle.boundingBox())!;
	const tb = (await target.boundingBox())!;
	await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2);
	await page.mouse.down();
	await page.mouse.move(tb.x + tb.width / 2, tb.y + tb.height / 2, { steps: 8 });
	await page.mouse.up();

	// The created connection carries size 'small' (frigate) from the heuristic.
	await expect
		.poll(async () => {
			const view = await (await api.get(`/api/maps/${mapId}`)).json();
			return view.connections[0]?.size ?? 'none';
		})
		.toBe('small');
	void from;
	void c13id;
});
