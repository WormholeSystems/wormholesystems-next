import { expect, gotoApp, test } from './fixtures';

// The head tags a link preview is built from. They have to be in the server's HTML, since
// that is all a crawler reads, and there has to be exactly one of each: two <title> tags
// means the browser takes the first, which is the one the page was trying to replace.

async function head(page: import('@playwright/test').Page, path: string) {
	const res = await page.request.get(path);
	return await res.text();
}

test('every page ships one set of preview tags', async ({ page }) => {
	for (const path of ['/', '/login']) {
		const html = await head(page, path);
		expect(html.match(/<title>/g)?.length, `${path} titles`).toBe(1);
		expect(html.match(/property="og:title"/g)?.length, `${path} og:title`).toBe(1);
		expect(html.match(/property="og:description"/g)?.length, `${path} og:description`).toBe(1);
		expect(html).toContain('name="twitter:card"');
	}
});

test('the card points at this host, not at a fixed one', async ({ page }) => {
	const html = await head(page, '/');
	// A self-hosted copy has to advertise itself, or the preview links somewhere the reader
	// cannot reach and loads an image that host never serves.
	expect(html).toContain('property="og:url" content="http://localhost:5173/"');
	expect(html).toContain('property="og:image" content="http://localhost:5173/og.png"');
});

test('a page can set its own title through its load', async ({ page }) => {
	await gotoApp(page, '/maps');
	await expect(page).toHaveTitle('Maps · WormholeSystems');
});

// A map link is what people paste at each other, so the preview says which map.
test('a map page is titled after the map', async ({ page, api }) => {
	const res = await api.post('/api/maps', { data: { name: 'E2E SeoMap' } });
	const mapId = (await res.json()).id as number;
	await gotoApp(page, `/maps/${mapId}`);
	await expect(page).toHaveTitle('E2E SeoMap · WormholeSystems');
});

// The mark is one colour, so a single icon disappears on one of the two themes. The SVG
// carries both and picks inside itself; the PNG behind it relies on the browser honouring
// `media` on the link, which is why the unconditional one has to be `no-preference` only.
test('the icon adapts to the theme', async ({ page }) => {
	const svg = await (await page.request.get('/favicon.svg')).text();
	expect(svg).toContain('prefers-color-scheme: dark');

	const html = await head(page, '/');
	expect(html).toContain('href="/favicon.svg"');
	expect(html).not.toMatch(/<link rel="icon"[^>]*href="\/favicon\.png"[^>]*>(?![^]*media)/);
	// No unconditional PNG icon, or it would match in dark mode and beat the dark one.
	for (const m of html.matchAll(/<link rel="icon"[^>]*href="\/favicon(-dark)?\.png"[^>]*>/g)) {
		expect(m[0]).toContain('media=');
	}
});

test('the icons and the card image are actually served', async ({ page }) => {
	for (const asset of ['/og.png', '/favicon.svg', '/favicon.png', '/favicon-dark.png', '/apple-touch-icon.png']) {
		const res = await page.request.get(asset);
		expect(res.status(), asset).toBe(200);
		expect(res.headers()['content-type'], asset).toContain('image/');
	}
});
