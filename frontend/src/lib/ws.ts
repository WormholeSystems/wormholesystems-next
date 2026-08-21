import type { MapEvent } from '$lib/api/types/MapEvent';
import type { UserEvent } from '$lib/api/types/UserEvent';

// Realtime sockets. Frames carry no payload we act on: each one only means "something
// changed, refetch". The map socket reconnects with backoff and reports its state, because
// a silently dead socket looks exactly like a quiet map.

function socketUrl(path: string): string {
	const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
	return `${scheme}://${location.host}${path}`;
}

export type SocketState = 'connecting' | 'open' | 'reconnecting';

const FIRST_RETRY_MS = 500;
const MAX_RETRY_MS = 15_000;

/**
 * Open the per-map event stream. `onEvent` fires per frame and once per successful reconnect,
 * with `null` for the reconnect catch-up, meaning "you missed something, reload everything".
 * Returns a close function that also stops the retry loop.
 */
export function openMapSocket(
	mapId: number,
	onEvent: (event: MapEvent | null) => void,
	onState?: (state: SocketState) => void,
): () => void {
	let ws: WebSocket | null = null;
	let retryMs = FIRST_RETRY_MS;
	let timer: ReturnType<typeof setTimeout> | null = null;
	let closed = false;
	// Only a reconnect needs the catch-up refetch; the first connect has the initial load.
	let reconnecting = false;

	function connect() {
		if (closed) return;
		onState?.(reconnecting ? 'reconnecting' : 'connecting');
		ws = new WebSocket(socketUrl(`/ws/map/${mapId}`));

		ws.onopen = () => {
			if (closed) return;
			retryMs = FIRST_RETRY_MS;
			onState?.('open');
			if (reconnecting) {
				reconnecting = false;
				onEvent(null);
			}
		};
		ws.onmessage = (frame) => {
			try {
				onEvent(JSON.parse(frame.data as string) as MapEvent);
			} catch {
				// Unreadable frame: fall back to a full refetch rather than ignoring it.
				onEvent(null);
			}
		};
		ws.onclose = () => {
			if (closed) return;
			reconnecting = true;
			onState?.('reconnecting');
			timer = setTimeout(connect, retryMs);
			retryMs = Math.min(retryMs * 2, MAX_RETRY_MS);
		};
		// An error is always followed by a close, which owns the retry.
		ws.onerror = () => ws?.close();
	}

	connect();

	return () => {
		closed = true;
		if (timer) clearTimeout(timer);
		ws?.close();
	};
}

/**
 * Open the per-user channel (activity heartbeat + status pings). Returns a close function.
 * Unlike the map socket the payload matters, since the channel mixes events addressed to this
 * user with news that concerns everyone.
 */
export function openUserSocket(onEvent: (event: UserEvent) => void): () => void {
	const ws = new WebSocket(socketUrl('/ws/user'));
	ws.onmessage = (frame) => {
		try {
			onEvent(JSON.parse(frame.data as string) as UserEvent);
		} catch {
			// A frame we cannot read is not worth acting on.
		}
	};
	return () => ws.close();
}
