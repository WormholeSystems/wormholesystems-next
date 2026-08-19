import type { Role } from '$lib/api/types/Role';

/** Matches the backend's `Role: Ord`, most privileged first. */
const RANK: Record<Role, number> = { owner: 0, manager: 1, member: 2, viewer: 3 };

/** No role at all is a guest watching a shared map, which is a viewer and nothing more. */
export function atLeast(role: Role | null | undefined, min: Role): boolean {
	return RANK[role ?? 'viewer'] <= RANK[min];
}

/** Owners-first ordering, for the access list. */
export function byRole(a: Role, b: Role): number {
	return RANK[a] - RANK[b];
}
