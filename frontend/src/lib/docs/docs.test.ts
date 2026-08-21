import { describe, expect, it } from 'vitest';
import { categories, pages, findPage } from './index';

// The pages are Markdown anyone can add, so what is checked here is the things a
// contributor can get wrong without noticing: a link to a page that moved, a title that
// never got written, two pages claiming the same URL.

describe('the documentation tree', () => {
	it('finds pages and orders them by their filename prefix', () => {
		expect(categories.length).toBeGreaterThan(0);
		expect(pages.length).toBeGreaterThan(0);
		expect(categories[0].slug).toBe('getting-started');
		expect(categories[0].pages[0].slug).toBe('overview');
	});

	it('gives every page a title and a unique url', () => {
		const urls = new Set<string>();
		for (const page of pages) {
			expect(page.title.trim(), page.url).not.toBe('');
			expect(urls.has(page.url), `duplicate ${page.url}`).toBe(false);
			urls.add(page.url);
		}
	});

	it('never links to a documentation page that does not exist', () => {
		const broken: string[] = [];
		for (const page of pages) {
			for (const [, href] of page.markdown.matchAll(/\]\((\/documentation\/[^)]+)\)/g)) {
				const [, , category, slug] = href.split('#')[0].split('/');
				if (!category || !slug || !findPage(category, slug)) broken.push(`${page.url} → ${href}`);
			}
		}
		expect(broken).toEqual([]);
	});
});
