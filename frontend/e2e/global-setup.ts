import { E2E_CHARACTER_ID, E2E_CORPORATION_ID, E2E_SESSION, withDb } from './db';

// Create (or refresh) the e2e identity: corp + user + character + session. Idempotent, so
// repeated runs reuse the same rows. The `users` id comes from the identity sequence —
// never forced — so real logins are unaffected.
export default async function globalSetup() {
	await withDb(async (db) => {
		await db.query(
			`insert into corporations (id, name, ticker) values ($1, 'E2E Corp', 'E2E')
			 on conflict do nothing`,
			[E2E_CORPORATION_ID],
		);
		const existing = await db.query('select user_id from characters where id = $1', [
			E2E_CHARACTER_ID,
		]);
		let userId: number = existing.rows[0]?.user_id;
		if (userId === undefined) {
			const user = await db.query('insert into users default values returning id');
			userId = user.rows[0].id;
			await db.query(
				`insert into characters (id, user_id, name, owner_hash, corporation_id)
				 values ($1, $2, 'E2E Pilot', 'e2e-owner-hash', $3)`,
				[E2E_CHARACTER_ID, userId, E2E_CORPORATION_ID],
			);
		}
		await db.query(
			`insert into sessions (id, user_id, active_character_id, expires_at)
			 values ($1, $2, $3, now() + interval '2 hours')
			 on conflict (id) do update set expires_at = now() + interval '2 hours'`,
			[E2E_SESSION, userId, E2E_CHARACTER_ID],
		);
	});
}
