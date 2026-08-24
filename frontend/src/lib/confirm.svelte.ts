// A promise-based confirmation, rendered by the ConfirmDialog host in the root layout.
// Replaces window.confirm so destructive flows keep their `if (!(await ...)) return`
// shape while the dialog itself is styled and e2e-addressable.

interface ConfirmRequest {
	title: string;
	body?: string;
	action: string;
	cancel: string;
	resolve: (answer: boolean) => void;
}

let pending = $state<ConfirmRequest | null>(null);

export function confirmDanger(opts: {
	title: string;
	body?: string;
	action?: string;
	cancel?: string;
}): Promise<boolean> {
	return new Promise((resolve) => {
		// A second ask while one is open answers the first with "no" rather than losing it.
		pending?.resolve(false);
		pending = {
			title: opts.title,
			body: opts.body,
			action: opts.action ?? 'Delete',
			cancel: opts.cancel ?? 'Cancel',
			resolve,
		};
	});
}

/** The host component's view of the pending request. */
export const confirmState = {
	get pending() {
		return pending;
	},
	settle(answer: boolean) {
		pending?.resolve(answer);
		pending = null;
	},
};
