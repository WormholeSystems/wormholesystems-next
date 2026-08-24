import { createMutation, useQueryClient, type QueryKey } from '@tanstack/svelte-query';
import { toast } from 'svelte-sonner';
import { errorMessage } from './client';

/**
 * The page-level write idiom: run the work, refetch what it touched, toast the failure.
 * The variables are the work itself, so one mutation serves every button on a page:
 * `act.mutate(() => api.revokeAccess({...}))`. Callers that sequence after success
 * (clearing a form, closing a dialog) use `mutateAsync`.
 */
export function apiAction(invalidates: () => readonly QueryKey[], onDone?: () => void) {
	const client = useQueryClient();
	return createMutation(() => ({
		mutationFn: (work: () => Promise<unknown>) => work(),
		onSuccess: async () => {
			await Promise.all(invalidates().map((queryKey) => client.invalidateQueries({ queryKey })));
			onDone?.();
		},
		onError: (err: unknown) => toast.error(errorMessage(err)),
	}));
}
