import { expect, gotoApp, test } from './fixtures';
import { clearKillmails, seedKillmail } from './db';

// The killmails card: what died in the mapped systems, newest first, with the expensive
// losses standing out.
//
// Rows are asserted by their own seeded ids rather than by count: the ingest is live
// against zKillboard, so a real kill can land in any system at any moment.

const J155207 = 31002402; // a second wormhole, so the busy-system test sees no real traffic
const J122515 = 31001882; // C5
const JITA = 30000142;
const AMARR = 30002187; // never placed on the test map

const SLASHER = 585;
const LOKI = 29990;

// Ids well outside anything zKillboard will hand us.
const RECENT = 900000001;
const OLD = 900000002;
const ELSEWHERE = 900000003;
const KSPACE = 900000004;
// A block of ids for the flooding test, well clear of the singles above.
const FLOOD = Array.from({ length: 55 }, (_, i) => 900001000 + i);
const QUIET = 900000005;
const SEEDED = [RECENT, OLD, ELSEWHERE, KSPACE, QUIET, ...FLOOD];

type Api = import('@playwright/test').APIRequestContext;

async function createMap(api: Api, name: string) {
	const res = await api.post('/api/maps', { data: { name } });
	expect(res.ok()).toBe(true);
	return (await res.json()).id as number;
}

async function addSystem(api: Api, mapId: number, solarSystemId: number, x: number) {
	const res = await api.post(`/api/maps/${mapId}/systems/add`, {
		data: { map_id: mapId, solar_system_id: solarSystemId, x, y: 200, alias: null }
	});
	expect(res.ok()).toBe(true);
}

test.afterEach(async () => {
	await clearKillmails(SEEDED);
});

test('recent kills in mapped systems are listed, and nothing else is', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E Kills');
	await addSystem(api, mapId, J122515, 200);
	await addSystem(api, mapId, JITA, 500);

	await seedKillmail({
		id: RECENT,
		solarSystemId: J122515,
		minutesAgo: 20,
		victimShipTypeId: LOKI,
		totalValue: 2_400_000_000,
		attackerCount: 4,
		finalBlowShipTypeId: SLASHER
	});
	// Older than the card's week-long window.
	await seedKillmail({
		id: OLD,
		solarSystemId: J122515,
		minutesAgo: 60 * 24 * 9,
		victimShipTypeId: SLASHER,
		totalValue: 5_000_000,
		attackerCount: 1
	});
	// A system nobody put on this map.
	await seedKillmail({
		id: ELSEWHERE,
		solarSystemId: AMARR,
		minutesAgo: 10,
		victimShipTypeId: SLASHER,
		totalValue: 5_000_000,
		attackerCount: 1
	});

	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	const card = page.getByTestId('killmails-card');
	await expect(card).toBeVisible();

	const recent = card.locator(`[data-kill="${RECENT}"]`);
	await expect(recent).toBeVisible({ timeout: 10_000 });
	await expect(recent).toContainText('Loki');
	await expect(recent).toContainText('J122515');
	await expect(recent.getByTestId('killmail-attackers')).toHaveText('4');
	// Over a billion, so it is called out rather than sitting in the muted column.
	await expect(recent.getByTestId('killmail-value')).toHaveText('2.4B');
	await expect(recent.getByTestId('killmail-value')).toHaveClass(/amber/);

	await expect(card.locator(`[data-kill="${OLD}"]`)).toHaveCount(0);
	await expect(card.locator(`[data-kill="${ELSEWHERE}"]`)).toHaveCount(0);
});

test('a solo kill is marked, and the filter narrows to one half of the chain', async ({
	page,
	api
}) => {
	const mapId = await createMap(api, 'E2E KillsFilter');
	await addSystem(api, mapId, J122515, 200);
	await addSystem(api, mapId, JITA, 500);

	await seedKillmail({
		id: RECENT,
		solarSystemId: J122515,
		minutesAgo: 5,
		victimShipTypeId: SLASHER,
		totalValue: 12_000_000,
		attackerCount: 1,
		isSolo: true
	});
	await seedKillmail({
		id: KSPACE,
		solarSystemId: JITA,
		minutesAgo: 6,
		victimShipTypeId: SLASHER,
		totalValue: 8_000_000,
		attackerCount: 3
	});

	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	const card = page.getByTestId('killmails-card');
	await expect(card.locator(`[data-kill="${RECENT}"]`)).toBeVisible({ timeout: 10_000 });
	await expect(card.locator(`[data-kill="${KSPACE}"]`)).toBeVisible();

	// One attacker: a hunter, not a fleet, and coloured to say so.
	await expect(
		card.locator(`[data-kill="${RECENT}"]`).getByTestId('killmail-attackers')
	).toHaveClass(/amber/);

	// Wormhole space only drops the Jita kill.
	await card.getByTestId('killmail-filter').click();
	await page.getByTestId('killmail-filter-jspace').click();
	await expect(card.locator(`[data-kill="${KSPACE}"]`)).toHaveCount(0, { timeout: 10_000 });
	await expect(card.locator(`[data-kill="${RECENT}"]`)).toBeVisible();

	// And it survives a reload, because it is stored against the user.
	await page.reload();
	await page.waitForSelector('[data-testid="panel-grid"]');
	await expect(card.locator(`[data-kill="${RECENT}"]`)).toBeVisible({ timeout: 10_000 });
	await expect(card.locator(`[data-kill="${KSPACE}"]`)).toHaveCount(0);

	// Put it back, so the next test starts from the default.
	await card.getByTestId('killmail-filter').click();
	await page.getByTestId('killmail-filter-all').click();
	await expect(card.locator(`[data-kill="${KSPACE}"]`)).toBeVisible({ timeout: 10_000 });
});

test('adding a system pulls in its kills straight away', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E KillsAdd');
	await addSystem(api, mapId, J122515, 200);

	// A kill in a system that is not on the map yet.
	await seedKillmail({
		id: KSPACE,
		solarSystemId: JITA,
		minutesAgo: 4,
		victimShipTypeId: SLASHER,
		totalValue: 9_000_000,
		attackerCount: 2
	});

	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	const card = page.getByTestId('killmails-card');
	await expect(card).toBeVisible();
	await expect(card.locator(`[data-kill="${KSPACE}"]`)).toHaveCount(0);

	// Putting the system on the map brings its history with it. The list is scoped to the
	// map's systems, so changing that set has to refetch, not wait for the next kill.
	await addSystem(api, mapId, JITA, 500);
	await expect(card.locator(`[data-kill="${KSPACE}"]`)).toBeVisible({ timeout: 15_000 });
});

test('a busy system cannot crowd the rest of the map out of the list', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E KillsFair');
	await addSystem(api, mapId, J122515, 200);
	await addSystem(api, mapId, J155207, 500);

	// One kill in the quiet system, then more recent kills in the other than the card has
	// room for. Ordered purely by time, every row would belong to the busy system and the
	// quiet one would vanish — which is exactly what a trade hub does to a real chain.
	await seedKillmail({
		id: QUIET,
		solarSystemId: J155207,
		minutesAgo: 90,
		victimShipTypeId: LOKI,
		totalValue: 3_000_000_000,
		attackerCount: 6
	});
	for (const [i, id] of FLOOD.entries()) {
		await seedKillmail({
			id,
			solarSystemId: J122515,
			minutesAgo: i + 1,
			victimShipTypeId: SLASHER,
			totalValue: 5_000_000,
			attackerCount: 2
		});
	}

	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	const card = page.getByTestId('killmails-card');
	await expect(card.locator(`[data-kill="${FLOOD[0]}"]`)).toBeVisible({ timeout: 10_000 });
	await expect(card.locator(`[data-kill="${QUIET}"]`)).toBeVisible();
});

test('a killmail row right-clicks to the system menu', async ({ page, api }) => {
	const mapId = await createMap(api, 'E2E KillsMenu');
	await addSystem(api, mapId, J122515, 200);
	await seedKillmail({
		id: RECENT,
		solarSystemId: J122515,
		minutesAgo: 3,
		victimShipTypeId: SLASHER,
		totalValue: 1_000_000,
		attackerCount: 2
	});

	await gotoApp(page, `/maps/${mapId}?system=${J122515}`);
	const row = page.getByTestId('killmails-card').locator(`[data-kill="${RECENT}"]`);
	await expect(row).toBeVisible({ timeout: 10_000 });

	await row.click({ button: 'right' });
	const menu = page.getByRole('menu');
	await expect(menu).toBeVisible();
	await expect(menu.getByText('Set destination')).toBeVisible();
});
