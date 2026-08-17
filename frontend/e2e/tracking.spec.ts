import { expect, test } from './fixtures';
import {
	E2E_CORPORATION_ID,
	createIdentity,
	grantAccess,
	setActiveCharacter,
	setCharacterPresence,
	withDb
} from './db';

// Jump tracking: flying through an unmapped hole builds it on the map, and the prompt
// asks which signature it turned out to be.
//
// Every test runs as its own pilot in its own context. The tracker watches the acting
// character's position, so sharing the main identity would leave it parked in a wormhole
// for whichever spec ran next.

const J122515 = 31001882; // C5, static H296
const J005482 = 31002515; // C2, shattered
const JITA = 30000142;
const PERIMETER = 30000144; // one gate from Jita

type Api = import('@playwright/test').APIRequestContext;
type Page = import('@playwright/test').Page;

async function createMap(api: Api, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

async function addSystem(api: Api, mapId: number, solarSystemId: number, alias: string | null) {
	const res = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: solarSystemId, x: 200, y: 200, alias }
	});
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

/** A pilot with their own session, map access, tracking on, and a starting location. */
async function openAsPilot(
	browser: import('@playwright/test').Browser,
	playwright: typeof import('@playwright/test'),
	slot: number,
	mapId: number,
	startSystem: number,
	settings: Record<string, boolean> = {}
) {
	const identity = await createIdentity(slot);
	await grantAccess(mapId, identity.characterId, 'member');
	await setCharacterPresence(identity.characterId, startSystem);

	const api = await playwright.request.newContext({
		baseURL: 'http://127.0.0.1:3000',
		extraHTTPHeaders: { cookie: `vector_session=${identity.session}` }
	});
	const res = await api.post(`/api/maps/${mapId}/settings/user`, {
		data: { tracking_allowed: true, prompt_for_signature: true, ...settings }
	});
	expect(res.ok()).toBe(true);

	const ctx = await browser.newContext();
	await ctx.addCookies([
		{ name: 'vector_session', value: identity.session, domain: 'localhost', path: '/' }
	]);
	const page = await ctx.newPage();
	await page.goto(`http://localhost:5173/maps/${mapId}?system=${startSystem}`);
	await page.waitForSelector('html[data-hydrated="true"]');
	await page.waitForSelector('[data-testid="panel-grid"]');

	async function close() {
		// Park the pilot offline, so a later spec never finds them sitting in a wormhole.
		await setCharacterPresence(identity.characterId, startSystem, false);
		await api.dispose();
		await ctx.close();
	}

	return { identity, page, close };
}

/**
 * Fly the pilot to a system, then hand the page the trigger it would get from the tab
 * regaining focus. That is the real path: the jump happens in the game client, so the map
 * only finds out when it next looks.
 */
async function jumpTo(page: Page, characterId: number, solarSystemId: number) {
	await setCharacterPresence(characterId, solarSystemId);
	await page.evaluate(() => window.dispatchEvent(new Event('focus')));
}

async function paste(page: Page, text: string) {
	await expect(page.getByTestId('signatures-card')).toBeVisible();
	await page.evaluate((t) => {
		const dt = new DataTransfer();
		dt.setData('text/plain', t);
		window.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt }));
	}, text);
}

async function graph(api: Api, mapId: number) {
	const view = await (await api.get(`/api/maps/${mapId}`)).json();
	return view as {
		systems: { id: number; solar_system_id: number; alias: string | null }[];
		connections: { id: number; from_system: number; to_system: number; kind: string }[];
	};
}

test('a jump through a scanned hole places the system, connects it and links the signature', async ({
	api,
	browser,
	playwright
}) => {
	const mapId = await createMap(api, 'E2E Tracking');
	await addSystem(api, mapId, J122515, null);
	const pilot = await openAsPilot(browser, playwright, 9, mapId, J122515);

	await paste(pilot.page, 'WHX-401\tCosmic Signature\tWormhole\t\t100%\t1 AU');
	await expect(pilot.page.getByTestId('sig-row')).toHaveCount(1);

	await jumpTo(pilot.page, pilot.identity.characterId, J005482);

	const dialog = pilot.page.getByTestId('tracking-dialog');
	await expect(dialog).toBeVisible();
	await expect(dialog.getByTestId('tracking-target')).toHaveText('J005482');
	// The lone candidate starts selected and the next chain alias is prefilled, so the
	// whole prompt is one keystroke.
	await expect(dialog.getByTestId('tracking-option')).toHaveCount(1);
	await expect(dialog.getByTestId('tracking-alias')).toHaveValue('1');
	await dialog.getByTestId('tracking-confirm').click();
	await expect(dialog).toBeHidden();

	await expect
		.poll(async () => (await graph(api, mapId)).systems.length, { timeout: 10_000 })
		.toBe(2);

	const view = await graph(api, mapId);
	expect(view.systems.find((s) => s.solar_system_id === J005482)?.alias).toBe('1');
	expect(view.connections).toHaveLength(1);
	expect(view.connections[0].kind).toBe('wormhole');

	const sigs = await (await api.get(`/api/maps/${mapId}/signatures`)).json();
	expect(sigs[0].connection_id).toBe(view.connections[0].id);
	// Jumped before it was ever typed, so linking promotes it out of "unknown".
	expect(sigs[0].group).toBe('wormhole');

	// The whole jump is one history step.
	await pilot.page.getByTestId('undo-button').click();
	await expect
		.poll(async () => (await graph(api, mapId)).systems.length, { timeout: 10_000 })
		.toBe(1);
	const undone = await (await api.get(`/api/maps/${mapId}/signatures`)).json();
	expect(undone).toHaveLength(1);
	expect(undone[0].connection_id).toBeNull();

	await pilot.close();
});

test('a gate hop builds nothing, and an unscanned jump maps the hole without asking', async ({
	api,
	browser,
	playwright
}) => {
	const mapId = await createMap(api, 'E2E TrackingGate');
	await addSystem(api, mapId, JITA, null);
	const pilot = await openAsPilot(browser, playwright, 10, mapId, JITA);

	await jumpTo(pilot.page, pilot.identity.characterId, PERIMETER);

	// Taking a gate is travel, not a discovery: no prompt, and nothing added.
	await expect(pilot.page.getByTestId('tracking-dialog')).toBeHidden();
	await pilot.page.waitForTimeout(500);
	expect((await graph(api, mapId)).systems).toHaveLength(1);

	// Back through the same gate, then out of Jita through a hole nobody scanned: with no
	// signature to offer there is nothing to ask, so it is mapped straight away.
	await jumpTo(pilot.page, pilot.identity.characterId, JITA);
	await jumpTo(pilot.page, pilot.identity.characterId, J122515);
	await expect
		.poll(async () => (await graph(api, mapId)).systems.length, { timeout: 10_000 })
		.toBe(2);
	await expect(pilot.page.getByTestId('tracking-dialog')).toBeHidden();
	expect((await graph(api, mapId)).connections).toHaveLength(1);

	await pilot.close();
});

test('switching character is not a jump', async ({ api, browser, playwright }) => {
	const mapId = await createMap(api, 'E2E TrackingSwitch');
	await addSystem(api, mapId, J122515, null);
	const pilot = await openAsPilot(browser, playwright, 11, mapId, J122515);

	// A second character on the same account, sitting somewhere else entirely.
	const altId = 91999980;
	await withDb((db) =>
		db.query(
			`insert into characters (id, user_id, name, owner_hash, corporation_id)
			 values ($1, $2, 'E2E Alt', 'e2e-alt-hash', $3)
			 on conflict (id) do nothing`,
			[altId, pilot.identity.userId, E2E_CORPORATION_ID]
		)
	);
	await setCharacterPresence(altId, JITA);

	// The watched system id changes because a different pilot is now active, not because
	// anyone flew. Reading it as a jump would invent a hole between two pilots' systems.
	await setActiveCharacter(altId, pilot.identity.session);
	await pilot.page.evaluate(() => window.dispatchEvent(new Event('focus')));

	await expect(pilot.page.getByTestId('tracking-dialog')).toBeHidden();
	await pilot.page.waitForTimeout(500);
	expect((await graph(api, mapId)).systems).toHaveLength(1);

	await setCharacterPresence(altId, JITA, false);
	await pilot.close();
});
