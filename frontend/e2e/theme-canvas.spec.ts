import { expect, gotoApp, test } from './fixtures';

// The map canvas is a painted surface, not a hole in the page: it has to follow the theme
// like everything else, or a light-mode map is a black rectangle with light grid lines.

test('the canvas and its grid follow the theme', async ({ page, api }) => {
	const res = await api.post('/api/maps', { data: { name: 'E2E Theme' } });
	const mapId = (await res.json()).id as number;
	await gotoApp(page, `/maps/${mapId}`);

	const canvas = page.getByTestId('map-canvas');
	const read = () =>
		canvas.evaluate((el) => {
			const world = el.querySelector('[style*="background-image"]') as HTMLElement;
			return {
				canvas: getComputedStyle(el).backgroundColor,
				grid: getComputedStyle(world).backgroundImage
			};
		});

	await page.evaluate(() => document.documentElement.classList.add('dark'));
	const dark = await read();
	await page.evaluate(() => document.documentElement.classList.remove('dark'));
	const light = await read();

	// Both halves move: the surface and the ruling drawn on it.
	expect(light.canvas).not.toBe(dark.canvas);
	expect(light.grid).not.toBe(dark.grid);
	// And light mode is actually light, rather than the dark canvas with pale lines on it.
	// Chrome reports these in the colour space they were written in, so read both forms.
	const lightness = (colour: string): number => {
		const oklch = colour.match(/oklch\(\s*([\d.]+)/);
		if (oklch) return Number(oklch[1]);
		const [r, g, b] = colour.match(/[\d.]+/g)!.map(Number);
		return (r + g + b) / 3 / 255;
	};
	expect(lightness(light.canvas)).toBeGreaterThan(0.8);
	expect(lightness(dark.canvas)).toBeLessThan(0.3);

	await api.delete(`/api/maps/${mapId}`);
});
