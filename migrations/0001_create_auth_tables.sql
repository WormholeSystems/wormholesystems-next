-- Authentication domain: users, characters, esi_tokens, esi_scopes, and the OAuth handshake.
-- See docs/database/authentication.md.

create table users (
    id            bigint generated always as identity primary key,
    last_active_at timestamptz,
    created_at    timestamptz not null default now()
);

create table characters (
    id             bigint primary key,                       -- EVE character id
    user_id        bigint not null references users (id) on delete cascade,
    name           text not null,
    owner_hash     text not null,                            -- changes on character transfer
    corporation_id bigint not null,
    alliance_id    bigint,
    is_preferred   boolean not null default false,
    updated_at     timestamptz not null default now()
);

-- At most one preferred character per user.
create unique index characters_one_preferred_per_user
    on characters (user_id)
    where is_preferred;

create table esi_tokens (
    id               bigint generated always as identity primary key,
    character_id     bigint not null references characters (id) on delete cascade,
    access_token     text,
    token_expires_at timestamptz,
    refresh_token    text not null,                          -- sensitive: encrypt at rest (TODO)
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
    code_verifier text,                                       -- PKCE (if used)
    link_user_id  bigint references users (id) on delete cascade,
    redirect_to   text,
    created_at    timestamptz not null default now(),
    expires_at    timestamptz not null
);
