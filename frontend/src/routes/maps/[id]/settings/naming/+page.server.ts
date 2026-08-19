import { userSettingsLoad } from '$lib/server/api';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = userSettingsLoad;
