-- App sessions: the server-side state behind the session cookie. Resolves the open
-- "session store" question in docs/database/authentication.md in favour of a Postgres
-- table (transactional, revocable), consistent with oauth_login_flows.
--
-- The cookie holds only the opaque `id`; everything authoritative lives here. The active
-- character is per-session (a user can be active as a different character on each device).

create table sessions (
    id                  text primary key,                         -- opaque random token (the cookie value)
    user_id             bigint not null references users (id) on delete cascade,
    active_character_id bigint not null references characters (id) on delete cascade,
    created_at          timestamptz not null default now(),
    expires_at          timestamptz not null
);

create index sessions_user_id on sessions (user_id);
