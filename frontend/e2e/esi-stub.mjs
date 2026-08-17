// A stand-in for ESI, so the e2e suite can fly a pilot around without a live EVE session.
//
// The point is to exercise the real path: the API's own poller asks this for a character's
// online state and location, writes `character_status`, publishes to the user channel and
// pushes down the socket. Faking the database row directly would skip all of that.
//
// Anything it has not been told about is proxied through to the real ESI, so it is safe to
// leave in front of a dev stack: your actual characters keep working.

import { createServer } from 'node:http';

const PORT = Number(process.env.ESI_STUB_PORT ?? 3999);
const UPSTREAM = process.env.ESI_STUB_UPSTREAM ?? 'https://esi.evetech.net';

/** character_id -> {online, solar_system_id, ship_type_id, ship_name, ship_item_id} */
const pilots = new Map();
/** Tranquility's own status, or null to let the real one through. */
let tranquility = null;
/** character_id -> how many times the API has asked about them, so tests can wait for a poll. */
const hits = new Map();

function json(res, status, body) {
	const payload = JSON.stringify(body);
	res.writeHead(status, {
		'content-type': 'application/json',
		'content-length': Buffer.byteLength(payload)
	});
	res.end(payload);
}

async function readJson(req) {
	const chunks = [];
	for await (const chunk of req) chunks.push(chunk);
	return chunks.length ? JSON.parse(Buffer.concat(chunks).toString()) : {};
}

/** Hand anything unscripted to the real ESI, headers and all. */
async function proxy(req, res, url) {
	try {
		const upstream = await fetch(`${UPSTREAM}${url.pathname}${url.search}`, {
			method: req.method,
			headers: {
				authorization: req.headers.authorization ?? '',
				'x-compatibility-date': req.headers['x-compatibility-date'] ?? ''
			}
		});
		const body = await upstream.text();
		res.writeHead(upstream.status, { 'content-type': 'application/json' });
		res.end(body);
	} catch (err) {
		json(res, 502, { error: `esi-stub upstream failed: ${err.message}` });
	}
}

const server = createServer(async (req, res) => {
	const url = new URL(req.url, `http://127.0.0.1:${PORT}`);

	// --- control plane, used by the tests ---

	if (url.pathname === '/_stub/reset' && req.method === 'POST') {
		pilots.clear();
		hits.clear();
		tranquility = null;
		return json(res, 200, { ok: true });
	}

	// Take Tranquility up, down, or into VIP. `unreachable` makes /status fail outright,
	// which is what the app sees when ESI itself is having a bad day.
	if (url.pathname === '/_stub/server' && req.method === 'PUT') {
		tranquility = await readJson(req);
		return json(res, 200, { ok: true });
	}
	if (url.pathname === '/_stub/server' && req.method === 'DELETE') {
		tranquility = null;
		return json(res, 200, { ok: true });
	}

	const control = url.pathname.match(/^\/_stub\/characters\/(\d+)$/);
	if (control && req.method === 'PUT') {
		const id = Number(control[1]);
		const body = await readJson(req);
		pilots.set(id, {
			online: body.online ?? true,
			solar_system_id: body.solar_system_id ?? null,
			ship_type_id: body.ship_type_id ?? 587,
			ship_name: body.ship_name ?? "Someone's Rifter",
			ship_item_id: body.ship_item_id ?? 1_000_000_000 + id
		});
		return json(res, 200, { ok: true });
	}
	if (control && req.method === 'DELETE') {
		pilots.delete(Number(control[1]));
		return json(res, 200, { ok: true });
	}

	// How many times the API has polled this character. Tests poll this to know the
	// backend is actually talking to the stub before asserting on what it did.
	const seen = url.pathname.match(/^\/_stub\/hits\/(\d+)$/);
	if (seen && req.method === 'GET') {
		return json(res, 200, { hits: hits.get(Number(seen[1])) ?? 0 });
	}

	// --- the ESI surface the app uses ---

	if (url.pathname === '/status' && req.method === 'GET') {
		if (!tranquility) return proxy(req, res, url);
		if (tranquility.unreachable) return json(res, 503, { error: 'ESI is in maintenance mode' });
		return json(res, 200, {
			players: tranquility.players ?? 0,
			server_version: tranquility.server_version ?? '2500000',
			start_time: tranquility.start_time ?? '2026-08-17T11:00:00Z',
			vip: tranquility.vip ?? false
		});
	}

	const character = url.pathname.match(/^\/characters\/(\d+)\/(online|location|ship)$/);
	if (character && req.method === 'GET') {
		const id = Number(character[1]);
		const pilot = pilots.get(id);
		if (!pilot) return proxy(req, res, url);

		hits.set(id, (hits.get(id) ?? 0) + 1);

		if (character[2] === 'online') {
			return json(res, 200, {
				online: pilot.online,
				last_login: null,
				last_logout: null,
				logins: 1
			});
		}
		// A pilot who has logged off has no location to report, which is what ESI does too.
		if (!pilot.online) return json(res, 403, { error: 'character not online' });
		if (character[2] === 'location') {
			return json(res, 200, {
				solar_system_id: pilot.solar_system_id,
				station_id: null,
				structure_id: null
			});
		}
		return json(res, 200, {
			ship_item_id: pilot.ship_item_id,
			ship_name: pilot.ship_name,
			ship_type_id: pilot.ship_type_id
		});
	}

	return proxy(req, res, url);
});

server.listen(PORT, '127.0.0.1', () => {
	console.log(`esi-stub listening on http://127.0.0.1:${PORT} (upstream ${UPSTREAM})`);
});
