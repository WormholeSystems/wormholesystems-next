-- Discord alerts: rules a map watches for, and where the resulting message goes.
--
-- One table rather than legacy's webhooks/roles/alerts split. That split is the scar of
-- two migrations away from a single bundled table, and the reuse it buys (naming a webhook
-- once and pointing several alerts at it) is not worth a join and two more editors.

create table discord_accounts (
    user_id         bigint primary key references users (id) on delete cascade,
    discord_user_id text        not null unique,
    username        text        not null,
    display_name    text,
    avatar          text,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);

-- A webhook the map has registered, pointed at by however many alerts want it.
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

-- A Discord role, named, so an alert can say who to ping without anyone reciting an id.
create table map_webhook_roles (
    id              bigint primary key generated always as identity,
    map_id          bigint      not null references maps (id) on delete cascade,
    name            text        not null,
    discord_role_id text        not null,
    created_at      timestamptz not null default now(),
    unique (map_id, discord_role_id)
);

create table map_alerts (
    id                     bigint primary key generated always as identity,
    map_id                 bigint      not null references maps (id) on delete cascade,
    -- Who to blame, and who to mention on `mention = 'creator'`. Kept when they leave so
    -- the alert survives; the lifecycle disables it separately if their access is gone.
    created_by_user_id     bigint references users (id) on delete set null,
    name                   text        not null,
    -- `killmail` | `proximity` | `jump_range`.
    kind                   text        not null,
    -- `webhook` | `discord_dm` | `discord_channel`.
    delivery               text        not null default 'webhook',
    discord_guild_id       text,
    discord_channel_id     text,
    -- Where it goes and who it pings, both named things the map registers once and points
    -- several alerts at. Pasting the same webhook URL into four alerts is four chances to
    -- paste the wrong one, and rotating it is four edits; a role gets a name because
    -- nobody knows which of theirs is 1189734502938472.
    map_webhook_id         bigint references map_webhooks (id) on delete cascade,
    map_webhook_role_id    bigint references map_webhook_roles (id) on delete set null,
    -- `none` | `creator` | `role` | `everyone`.
    mention                text        not null default 'none',
    -- Proximity and jump range only: what the chain has to come close to.
    target_solar_system_id bigint references solar_systems (id) on delete cascade,
    -- Jump range only: `dreadnought` | `carrier` | `force_auxiliary` | `supercarrier`
    -- | `titan` | `jump_freighter` | `rorqual` | `black_ops`, and the pilot's JDC level.
    ship_type              text,
    jdc_level              integer,
    -- Gate jumps for proximity and killmail; light years, tenths, for jump range.
    max_jumps              integer     not null default 5,
    -- Killmail only: `[{subject, side, mode, ids}]`, matched per `filter_match`.
    filters                jsonb       not null default '[]'::jsonb,
    -- `any` | `all`. Excludes veto regardless.
    filter_match           text        not null default 'any',
    is_active              boolean     not null default true,
    last_fired_at          timestamptz,
    disabled_at            timestamptz,
    -- `manual` | `discord_unlinked` | `access_revoked` | `destination_gone` | `delivery_failed`.
    disabled_reason        text,
    created_at             timestamptz not null default now(),
    updated_at             timestamptz not null default now()
);

create index map_alerts_active on map_alerts (kind, is_active) where is_active;
create index map_alerts_map on map_alerts (map_id);

-- The ledger that makes delivery idempotent. A row is reserved before the message is sent,
-- so a retry of a half-finished send finds the reservation and stops. The key is whatever
-- identifies the occasion: the placement for proximity, the killmail for a kill.
create table map_alert_deliveries (
    id           bigint primary key generated always as identity,
    map_alert_id bigint      not null references map_alerts (id) on delete cascade,
    dedup_key    text        not null,
    delivered_at timestamptz,
    created_at   timestamptz not null default now(),
    unique (map_alert_id, dedup_key)
);

create index map_alert_deliveries_age on map_alert_deliveries (created_at);

-- What has happened to an alert, for the settings page's audit trail: who made it, who
-- turned it off, and every time it fired or failed.
create table map_alert_events (
    id            bigint primary key generated always as identity,
    map_alert_id  bigint references map_alerts (id) on delete set null,
    map_id        bigint      not null,
    actor_user_id bigint references users (id) on delete set null,
    -- `created` | `updated` | `enabled` | `disabled` | `deleted` | `fired` | `failed`.
    kind          text        not null,
    detail        text,
    created_at    timestamptz not null default now()
);

create index map_alert_events_map on map_alert_events (map_id, created_at desc);
