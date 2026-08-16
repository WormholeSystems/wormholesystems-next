// Realtime sockets. Same semantics as the old client: fire-and-forget, no reconnect;
// the map socket's frames are only a "something changed" signal driving a refetch.

function socketUrl(path: string): string {
	const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
	return `${scheme}://${location.host}${path}`;
}

/** Open the per-map event stream; any frame means "refetch". Returns a close function. */
export function openMapSocket(mapId: number, onEvent: () => void): () => void {
	const ws = new WebSocket(socketUrl(`/ws/map/${mapId}`));
	ws.onmessage = () => onEvent();
	return () => ws.close();
}

/** Open the per-user channel (activity heartbeat + status pings). Returns a close function. */
export function openUserSocket(onEvent: () => void): () => void {
	const ws = new WebSocket(socketUrl('/ws/user'));
	ws.onmessage = () => onEvent();
	return () => ws.close();
}
