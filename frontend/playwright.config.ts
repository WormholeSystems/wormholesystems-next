import { defineConfig } from '@playwright/test';

// E2E tests run against the real stack: Postgres (docker compose up db), the Rust API,
// and the vite dev server. Both servers are reused when already running (the solo dev
// setup), so locally the tests attach to your running stack.
export default defineConfig({
	testDir: 'e2e',
	use: {
		baseURL: 'http://localhost:5173'
	},
	globalSetup: './e2e/global-setup',
	globalTeardown: './e2e/global-teardown',
	webServer: [
		{
			command: 'cargo run',
			cwd: '..',
			url: 'http://127.0.0.1:3000/api/grid-config',
			reuseExistingServer: true,
			// First run compiles the API and may seed the SDE into a fresh database.
			timeout: 300_000
		},
		{
			command: 'npm run dev',
			url: 'http://localhost:5173',
			reuseExistingServer: true,
			timeout: 60_000
		}
	]
});
