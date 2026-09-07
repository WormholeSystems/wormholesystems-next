-- Bearer tokens a user mints for scripts and integrations. Only the hash is kept: the
-- plaintext is shown once at creation, so a copy of this table is not a copy of the keys.
create table personal_access_tokens (
    id           bigint generated always as identity primary key,
    user_id      bigint not null references users (id) on delete cascade,
    name         text not null,
    token_hash   text not null unique,
    last_used_at timestamptz,
    -- Null lasts until revoked.
    expires_at   timestamptz,
    created_at   timestamptz not null default now()
);

create index personal_access_tokens_user_id on personal_access_tokens (user_id);
