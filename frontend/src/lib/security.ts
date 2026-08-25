// Security status the way EVE rounds it, ported from Nohus' CCPRounding class
// (https://gitlab.com/rift-intel-fusion-tool/) via the legacy app. Every comparison or
// band check on a security status goes through this first; raw SDE values sit slightly
// off the displayed tenths (Uedama is 0.46) and comparing them directly misclassifies.

/**
 * Round a raw security status to the value the game shows: exactly 0.0 stays 0.0,
 * anything in (0, 0.05) rounds up to 0.1 (a positive true sec is always at least
 * lowsec), everything else rounds to the nearest tenth, halves away from zero.
 */
export function ccpRoundSecurity(security: number): number {
	if (security === 0) return 0;
	if (security > 0 && security < 0.05) return 0.1;
	const scaled = security * 10;
	return (Math.sign(scaled) * Math.round(Math.abs(scaled))) / 10;
}
