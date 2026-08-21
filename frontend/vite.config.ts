import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

// In dev the Axum API runs separately; proxy the backend paths so the whole app
// (including the EVE SSO flow, Discord linking and both WebSockets) lives on the vite
// origin.
const backend = 'http://127.0.0.1:3000';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true,
			},
			adapter: adapter(),
		}),
	],
	server: {
		proxy: {
			'/api': backend,
			'/auth': backend,
			'/discord': backend,
			'/ws': { target: backend, ws: true },
		},
	},
	test: {
		// Unit tests cover pure logic only. `e2e/` is Playwright's and must stay out of
		// vitest's reach, or it collects those specs and fails on the wrong runner.
		include: ['src/**/*.test.ts'],
	},
});
