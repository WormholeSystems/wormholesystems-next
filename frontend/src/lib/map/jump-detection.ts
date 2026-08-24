// Telling a jump apart from everything else that moves the watched system id: switching
// character, logging in or out, the first reading of a session. Anything ambiguous is
// rejected rather than guessed at.

/** One reading of the acting character's position. */
export interface Reading {
	characterId: number | null;
	/** Null when offline or unknown: a stale location is not a location. */
	systemId: number | null;
}

/**
 * Two consecutive readings of the same character in two different systems is the only
 * shape a jump can have. Anything else is null.
 */
export function detectJump(prev: Reading, next: Reading): { from: number; to: number } | null {
	// A missing reading on either side is a login, a logout, or the first poll of the
	// session. None of them is a jump, and the last known system may be hours stale.
	if (prev.systemId === null || next.systemId === null) return null;
	if (prev.systemId === next.systemId) return null;
	// Switching character moves the watched system id without anyone flying anywhere;
	// acting on it would connect two pilots' systems.
	if (prev.characterId !== next.characterId) return null;
	return { from: prev.systemId, to: next.systemId };
}
