// Dark/light mode, persisted under the same localStorage key the old app used. The
// initial state was already applied to <html> by the blocking script in app.html.

import { browser } from '$app/environment';

function initialDark(): boolean {
	if (!browser) return false;
	return document.documentElement.classList.contains('dark');
}

class Theme {
	dark = $state(initialDark());

	toggle() {
		this.dark = !this.dark;
		localStorage.setItem('darkmode', String(this.dark));
		document.documentElement.classList.toggle('dark', this.dark);
	}
}

export const theme = new Theme();
