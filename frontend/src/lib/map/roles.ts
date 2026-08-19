import type { Role } from '$lib/api/types/Role';

/** Matches the backend's `Role: Ord`, most privileged first. */
const RANK: Record<Role, number> = { owner: 0, manager: 1, member: 2, viewer: 3 };

/** Nobody is a guest watching a shared map, which is a viewer and nothing more. */
export function atLeast(role: Role | null | undefined, min: Role): boolean {
	return RANK[role ?? 'viewer'] <= RANK[min];
}

export function canWrite(role: Role | null | undefined): boolean {
	return atLeast(role, 'member');
}

export function canManage(role: Role | null | undefined): boolean {
	return atLeast(role, 'manager');
}

/** Owners-first ordering, for the access list. */
export function byRole(a: Role, b: Role): number {
	return RANK[a] - RANK[b];
}
