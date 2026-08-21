import { marked } from 'marked';

/**
 * The documentation tree, built at compile time from the Markdown under `src/docs/`.
 * Folders are categories and files are pages; the `NN-` prefix on either orders it and is
 * dropped from the URL, so contributing a page means adding a file and nothing else.
 */
const files = import.meta.glob<string>('/src/docs/**/*.md', {
	query: '?raw',
	import: 'default',
	eager: true,
});

export interface DocPage {
	title: string;
	slug: string;
	url: string;
	category: string;
	categorySlug: string;
	markdown: string;
}

export interface DocCategory {
	title: string;
	slug: string;
	pages: DocPage[];
}

/** `03-bookmarking` → `bookmarking`, which is what the URL and the sidebar both use. */
function unprefix(name: string): string {
	return name.replace(/^\d+-/, '');
}

/** `getting-started` → `Getting started`, for a folder that named no category of its own. */
function humanise(slug: string): string {
	const words = slug.replaceAll('-', ' ');
	return words.charAt(0).toUpperCase() + words.slice(1);
}

/** A page split into what it declares about itself and what it says. */
interface Parsed {
	data: Record<string, string>;
	body: string;
}

/** The frontmatter block, which is only ever a handful of `key: value` lines. */
function frontmatter(source: string): Parsed {
	const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/.exec(source);
	if (!match) return { data: {}, body: source };
	const data: Record<string, string> = {};
	for (const line of match[1].split(/\r?\n/)) {
		const at = line.indexOf(':');
		if (at === -1) continue;
		data[line.slice(0, at).trim()] = line
			.slice(at + 1)
			.trim()
			.replace(/^["']|["']$/g, '');
	}
	return { data, body: source.slice(match[0].length) };
}

/** The first `# Heading`, for a page that did not title itself. */
function firstHeading(body: string): string | null {
	return /^#\s+(.+)$/m.exec(body)?.[1]?.trim() ?? null;
}

/**
 * The tree while it is being built: the `NN-` prefixes decide the order and are then of no
 * further interest, so they live here rather than on the pages everything else sees.
 */
interface SortedPage extends DocPage {
	order: string;
}

interface SortedCategory {
	title: string;
	slug: string;
	order: string;
	pages: SortedPage[];
}

function buildTree(): DocCategory[] {
	const byCategory = new Map<string, SortedCategory>();

	for (const [path, source] of Object.entries(files)) {
		// `/src/docs/03-bookmarking/02-k-space-connections.md`. Anything not exactly one
		// folder deep is not a page: the directory's own README documents the convention.
		const parts = path.replace('/src/docs/', '').split('/');
		if (parts.length !== 2) continue;
		const [folder, file] = parts;
		const { data, body } = frontmatter(source);

		const categorySlug = unprefix(folder);
		const category = byCategory.get(categorySlug) ?? {
			title: data.category ?? humanise(categorySlug),
			slug: categorySlug,
			pages: [],
			order: folder,
		};
		if (data.category) category.title = data.category;
		byCategory.set(categorySlug, category);

		const slug = unprefix(file.replace(/\.md$/, ''));
		const page: SortedPage = {
			title: data.title ?? firstHeading(body) ?? humanise(slug),
			slug,
			url: `/documentation/${categorySlug}/${slug}`,
			category: category.title,
			categorySlug,
			markdown: body,
			order: file,
		};
		category.pages.push(page);
	}

	return [...byCategory.values()]
		.sort((a, b) => a.order.localeCompare(b.order))
		.map((category) => ({
			title: category.title,
			slug: category.slug,
			pages: category.pages.sort((a, b) => a.order.localeCompare(b.order)),
		}));
}

export const categories: DocCategory[] = buildTree();

/** Every page in sidebar order, which is also the order prev/next walk. */
export const pages: DocPage[] = categories.flatMap((category) => category.pages);

export function findPage(categorySlug: string, slug: string): DocPage | null {
	return pages.find((p) => p.categorySlug === categorySlug && p.slug === slug) ?? null;
}

/** The pages either side of one, which is what the footer links to. */
export interface Neighbours {
	prev: DocPage | null;
	next: DocPage | null;
}

export function neighbours(page: DocPage): Neighbours {
	const at = pages.indexOf(page);
	return { prev: pages[at - 1] ?? null, next: pages[at + 1] ?? null };
}

/** Markdown to HTML. Raw HTML in a page is escaped rather than rendered. */
export function render(markdown: string): string {
	// SAFETY: `marked.parse` returns a promise only when `async` is set, and it is not.
	const html = marked.parse(markdown, { async: false }) as string;
	return html.replaceAll('<a href="http', '<a target="_blank" rel="noopener" href="http');
}
