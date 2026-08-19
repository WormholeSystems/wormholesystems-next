// Every action a person can take on a map, and what the map says about it.
//
// One catalogue rather than a phrase at each call site, so the wording is consistent, the
// silent ones are silent on purpose, and adding an action means deciding here what it says
// rather than inventing a sentence in a click handler.
//
// The rule for `done`: say something only when the map does not. Dragging a node, renaming
// one, or removing a connection are all visible where they happened, and announcing them
// would bury the ones worth reading. What earns a line is work that lands somewhere you
// are not looking: the clipboard, the EVE client, the far end of a long undo.

export interface ActionCopy {
	/** What went wrong, in words. The server's message rides along as the detail. */
	failed: string;
	/** Said on success, for the actions whose result is not on the screen. */
	done?: string;
}

export const MAP_ACTIONS = {
	// --- the graph ---
	addSystem: { failed: 'Could not add the system' },
	moveSystems: { failed: 'Could not move the systems' },
	removeSystems: { failed: 'Could not remove the systems' },
	clearMap: { failed: 'Could not clear the map', done: 'Map cleared' },
	// Bulk and destructive, and what went is a branch you were not looking at.
	cleanMap: { failed: 'Could not clean the map', done: 'Map cleaned' },
	setAlias: { failed: 'Could not rename the system' },
	setStatus: { failed: 'Could not set the status' },
	setOccupier: { failed: 'Could not set the occupier' },
	setPinned: { failed: 'Could not pin the system' },
	setHome: { failed: 'Could not set the home system' },
	setRally: { failed: 'Could not set the rally point' },
	setNotes: { failed: 'Could not save the notes' },
	assignSystem: { failed: 'Could not assign the system' },

	// --- connections ---
	addConnection: { failed: 'Could not connect the systems' },
	removeConnection: { failed: 'Could not remove the connection' },
	setConnectionType: { failed: 'Could not change the connection type' },
	setConnectionSize: { failed: 'Could not set the ship size' },
	setConnectionMass: { failed: 'Could not set the mass' },
	setConnectionLifetime: { failed: 'Could not set the lifetime' },
	setPreserveMass: { failed: 'Could not change mass tracking' },
	cleanStale: { failed: 'Could not clean the stale connections', done: 'Stale connections cleaned' },
	addJump: { failed: 'Could not log the jump' },
	updateJump: { failed: 'Could not update the jump' },
	removeJump: { failed: 'Could not remove the jump' },
	trackJump: { failed: 'Could not map the jump' },

	// --- signatures ---
	pasteSignatures: { failed: 'Could not paste the signatures' },
	addSignature: { failed: 'Could not add the signature' },
	updateSignature: { failed: 'Could not update the signature' },
	removeSignature: { failed: 'Could not remove the signature' },
	removeMissingSignatures: { failed: 'Could not remove the missing signatures' },
	linkSignature: { failed: 'Could not link the signature' },
	unlinkSignature: { failed: 'Could not unlink the signature' },

	// --- the map's own state ---
	watch: { failed: 'Could not add it to the watchlist' },
	unwatch: { failed: 'Could not remove it from the watchlist' },
	// The change may be anywhere on the map, including off screen, so these say so.
	// These three carry the step they walked past as their detail, so the toast says what
	// moved rather than only that something did.
	undo: { failed: 'Could not undo', done: 'Undone' },
	redo: { failed: 'Could not redo', done: 'Redone' },
	goToEvent: { failed: 'Could not go to that point', done: 'Map moved back to' },
	saveLayout: { failed: 'Could not save the layout', done: 'Layout saved' },
	setPlacement: { failed: 'Could not change the placement' },

	// Lands in the EVE client, where the app cannot show you anything.
	setWaypoint: { failed: 'Could not set the destination', done: 'Destination set' }
} as const satisfies Record<string, ActionCopy>;

export type MapAction = keyof typeof MAP_ACTIONS;
