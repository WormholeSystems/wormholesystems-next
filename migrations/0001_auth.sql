-- Conventions used across every migration here:
--   * Enum-like columns are `text`, validated by Rust enums rather than PG enum types, so
--     adding a value is a code change rather than a migration.
--   * SDE ids are `bigint` everywhere, which avoids FK type mismatches and large-id
--     overflow.
--   * Foreign keys between the ESI-cached entity tables are DEFERRABLE INITIALLY DEFERRED:
--     they form a real cycle (a faction points at its NPC corp, which points back), so a
--     seeder inserts the lot in one transaction and they are checked at commit.
--
-- Each file is the finished shape of one domain rather than a history of edits: the app's
-- incremental migrations were squashed before any of this ran in production, so nothing
-- was preserved that an existing database needed.
--
-- See docs/database/ for the domain-by-domain description.

-- ---------------------------------------------------------------------------
-- Authentication
-- ---------------------------------------------------------------------------

create table users (
    id             bigint generated always as identity primary key,
    last_active_at timestamptz,
    created_at     timestamptz not null default now()
);

-- Every character we have had to know about, whether or not anyone signed in as one.
--
-- `user_id` is what separates the two kinds. A row with one is a character somebody
-- authenticated with, and carries the auth columns to prove it. A row without one is a
-- character we merely had to name: a killmail victim, the pilot who landed a final blow.
-- Everything that cares whose character it is already filters on `user_id`, so the two
-- kinds never get confused for one another.
create table characters (
    id             bigint primary key,                       -- EVE character id
    user_id        bigint references users (id) on delete cascade,
    name           text not null,
    owner_hash     text,                                     -- changes on character transfer
    corporation_id bigint,
    alliance_id    bigint,
    is_preferred   boolean not null default false,
    updated_at     timestamptz not null default now()
);

-- At most one preferred character per user.
create unique index characters_one_preferred_per_user
    on characters (user_id)
    where is_preferred;

-- The refresh token is stored as it comes back from EVE, not encrypted. Anyone who can
-- read this table can act as any linked character within the scopes it was granted, so the
-- database is exactly as sensitive as the characters on it. Encrypting it would need a key
-- kept somewhere the application can reach and an attacker cannot, which on a single-host
-- deployment is largely a matter of moving the problem; saying plainly what the property is
-- beats implying it is solved.
create table esi_tokens (
    id               bigint generated always as identity primary key,
    character_id     bigint not null references characters (id) on delete cascade,
    access_token     text,
    token_expires_at timestamptz,
    refresh_token    text not null,
    created_at       timestamptz not null default now(),
    updated_at       timestamptz not null default now()
);

create table esi_scopes (
    id          bigint generated always as identity primary key,
    name        text not null unique,
    description text
);

create table esi_token_scopes (
    token_id bigint not null references esi_tokens (id) on delete cascade,
    scope_id bigint not null references esi_scopes (id) on delete restrict,
    primary key (token_id, scope_id)
);

create table oauth_login_flows (
    state         text primary key,                          -- CSRF token echoed by the SSO
    code_verifier text,                                      -- PKCE (if used)
    link_user_id  bigint references users (id) on delete cascade,
    redirect_to   text,
    created_at    timestamptz not null default now(),
    expires_at    timestamptz not null
);

-- App sessions: the server-side state behind the session cookie. The cookie holds only
-- the opaque `id`; everything authoritative lives here. The active character is
-- per-session, so a user can be a different character on each device.
create table sessions (
    id                  text primary key,                    -- opaque random token (the cookie value)
    user_id             bigint not null references users (id) on delete cascade,
    active_character_id bigint not null references characters (id) on delete cascade,
    created_at          timestamptz not null default now(),
    expires_at          timestamptz not null
);

create index sessions_user_id on sessions (user_id);
