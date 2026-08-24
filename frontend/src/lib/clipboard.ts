import { toast } from 'svelte-sonner';

/** Put `text` on the clipboard; false (and a toast, unless silent) when the browser refuses. */
export async function copyText(
	text: string,
	opts: { success?: string; description?: string; silent?: boolean } = {},
): Promise<boolean> {
	try {
		await navigator.clipboard.writeText(text);
		if (opts.success) toast.success(opts.success, { description: opts.description });
		return true;
	} catch {
		if (!opts.silent) toast.error('Clipboard access denied');
		return false;
	}
}

/** Read the clipboard; null (and a toast) when the browser refuses. */
export async function readText(): Promise<string | null> {
	try {
		if (!navigator.clipboard?.readText) throw new Error('unavailable');
		return await navigator.clipboard.readText();
	} catch {
		toast.error('Clipboard access denied');
		return null;
	}
}
