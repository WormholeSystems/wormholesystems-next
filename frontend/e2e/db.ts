// Direct database access for test fixtures. The app has no test-only endpoints, so the
// e2e identity (corp + user + character + session) is created straight in Postgres.
import { Client } from 'pg';

export const E2E_CORPORATION_ID = 98999998;
export const E2E_CHARACTER_ID = 91999998;
export const E2E_SESSION = 'e2e-session';

export async function withDb<T>(f: (client: Client) => Promise<T>): Promise<T> {
	const client = new Client({
		connectionString:
			process.env.DATABASE_URL ?? 'postgres://vector:vector@localhost:5432/vector'
	});
	await client.connect();
	try {
		return await f(client);
	} finally {
		await client.end();
	}
}

/**
 * Create (or refresh) an extra throwaway identity for multi-user tests. Ids are derived
 * from `slot` (0-9); global teardown does not know about these, so callers should clean
 * up maps they create, and the rows themselves are reused across runs.
 */
export async function createIdentity(slot: number): Promise<{
	characterId: number;
	session: string;
	userId: number;
}> {
	const characterId = 91999900 + slot;
	const session = `e2e-extra-${slot}`;
	const userId = await withDb(async (db) => {
		await db.query(
			`insert into corporations (id, name, ticker) values ($1, 'E2E Corp', 'E2E')
			 on conflict do nothing`,
			[E2E_CORPORATION_ID]
		);
		const existing = await db.query('select user_id from characters where id = $1', [characterId]);
		let userId: number = existing.rows[0]?.user_id;
		if (userId === undefined) {
			const user = await db.query('insert into users default values returning id');
			userId = user.rows[0].id;
			await db.query(
				`insert into characters (id, user_id, name, owner_hash, corporation_id)
				 values ($1, $2, $3, $4, $5)`,
				[characterId, userId, `E2E Extra ${slot}`, `e2e-extra-hash-${slot}`, E2E_CORPORATION_ID]
			);
		}
		await db.query(
			`insert into sessions (id, user_id, active_character_id, expires_at)
			 values ($1, $2, $3, now() + interval '2 hours')
			 on conflict (id) do update set expires_at = now() + interval '2 hours'`,
			[session, userId, characterId]
		);
		return userId;
	});
	return { characterId, session, userId };
}

/** Grant a character access to a map directly (no invite flow exists yet). */
export async function grantAccess(
	mapId: number,
	characterId: number,
	role: 'viewer' | 'member' | 'manager' | 'owner'
) {
	await withDb(async (db) => {
		await db.query(
			`insert into map_access (map_id, subject_type, subject_id, role)
			 values ($1, 'character', $2, $3)
			 on conflict (map_id, subject_id) do update set role = excluded.role`,
			[mapId, characterId, role]
		);
	});
}

/** Give a character a tracking-scoped token and an online status in a system. */
export async function setCharacterPresence(
	characterId: number,
	solarSystemId: number,
	online = true
) {
	await withDb(async (db) => {
		const scope = await db.query(
			`insert into esi_scopes (name) values ('esi-location.read_location.v1')
			 on conflict (name) do update set name = excluded.name returning id`
		);
		const token = await db.query(
			`insert into esi_tokens (character_id, refresh_token) values ($1, 'e2e-token')
			 returning id`,
			[characterId]
		);
		await db.query(
			`insert into esi_token_scopes (token_id, scope_id) values ($1, $2)
			 on conflict do nothing`,
			[token.rows[0].id, scope.rows[0].id]
		);
		await db.query(
			`insert into character_status (character_id, online, solar_system_id, last_online_at)
			 values ($1, $2, $3, now())
			 on conflict (character_id) do update set
			     online = excluded.online, solar_system_id = excluded.solar_system_id,
			     updated_at = now()`,
			[characterId, online, solarSystemId]
		);
	});
}

/** Set a user's per-map tracking opt-in directly. */
export async function setTrackingAllowed(mapId: number, userId: number, allowed: boolean) {
	await withDb(async (db) => {
		await db.query(
			`insert into map_user_settings (map_id, user_id, tracking_allowed)
			 values ($1, $2, $3)
			 on conflict (map_id, user_id) do update set tracking_allowed = excluded.tracking_allowed`,
			[mapId, userId, allowed]
		);
	});
}

/** Write a wormhole system's threat analysis directly (as the daily batch would). */
export async function setThreat(
	solarSystemId: number,
	level: 'unknown' | 'high' | 'critical',
	entities: { id: number; type: 'alliance' | 'corporation'; name: string; kills: number }[]
) {
	await withDb(async (db) => {
		await db.query(
			`update wormhole_systems set threat_level = $2, threat_analyzed_at = now()
			 where solar_system_id = $1`,
			[solarSystemId, level]
		);
		await db.query('delete from wormhole_system_threats where solar_system_id = $1', [
			solarSystemId
		]);
		for (const e of entities) {
			await db.query(
				`insert into wormhole_system_threats (solar_system_id, entity_id, entity_type, name, kills)
				 values ($1, $2, $3, $4, $5)`,
				[solarSystemId, e.id, e.type, e.name, e.kills]
			);
		}
	});
}
