import { createMutation, useQueryClient, type QueryKey } from '@tanstack/svelte-query';
import { toast } from 'svelte-sonner';
import { errorMessage } from './client';

/**
 * The page-level write idiom: run the work, refetch what it touched, toast the failure
 * (and optionally the success). The variables are the work itself, so one mutation serves
 * every button on a page: `act.mutate(() => api.revokeAccess({...}))`. Callers that
 * sequence after success use `mutateAsync`, usually through [`after`].
 */
export function apiAction(
	invalidates: () => readonly QueryKey[],
	opts: (() => void) | { onDone?: () => void; success?: string } = {},
) {
	const { onDone, success } = typeof opts === 'function' ? { onDone: opts } : opts;
	const client = useQueryClient();
	return createMutation(() => ({
		mutationFn: (work: () => Promise<unknown>) => work(),
		onSuccess: async () => {
			await Promise.all(invalidates().map((queryKey) => client.invalidateQueries({ queryKey })));
			if (success) toast.success(success);
			onDone?.();
		},
		onError: (err: unknown) => toast.error(errorMessage(err)),
	}));
}

/**
 * Sequence after an apiAction's `mutateAsync` settles. The failure is already toasted by
 * the mutation, so the rejection is swallowed rather than left unhandled.
 */
export function after<T>(promise: Promise<T>, onSuccess: (value: T) => void): void {
	promise.then(onSuccess).catch(() => {});
}
