-- Reshaped to match how alerts are actually set up.
--
-- Destinations and roles are named things a map registers once and points several alerts
-- at, not fields repeated on every alert. Pasting the same webhook URL into four alerts is
-- four chances to paste the wrong one, and rotating it is four edits. Roles get names for
-- the same reason: nobody knows which of their roles is 1189734502938472.
--
-- Jump range needs the coordinates the SDE has always carried, and the ship it is being
-- measured for: "within range" means nothing without a hull and a JDC level.

alter table solar_systems
    add column pos_x double precision,
    add column pos_y double precision,
    add column pos_z double precision;

create table map_webhooks (
    id         bigint primary key generated always as identity,
    map_id     bigint      not null references maps (id) on delete cascade,
    name       text        not null,
    -- Write-only: the API returns a redacted summary, never this.
    url        text        not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (map_id, name)
);

create table map_webhook_roles (
    id              bigint primary key generated always as identity,
    map_id          bigint      not null references maps (id) on delete cascade,
    name            text        not null,
    discord_role_id text        not null,
    created_at      timestamptz not null default now(),
    unique (map_id, discord_role_id)
);

alter table map_alerts
    drop column webhook_url,
    drop column discord_role_id,
    add column map_webhook_id bigint references map_webhooks (id) on delete cascade,
    add column map_webhook_role_id bigint references map_webhook_roles (id) on delete set null,
    -- Jump range only: `dreadnought` | `carrier` | `force_auxiliary` | `supercarrier`
    -- | `titan` | `jump_freighter` | `rorqual` | `black_ops`.
    add column ship_type text,
    add column jdc_level integer;
