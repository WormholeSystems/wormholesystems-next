import { currentCharacter } from '$lib/server/api';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async (event) => {
	return { me: await currentCharacter(event) };
};
