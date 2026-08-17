-- ---------------------------------------------------------------------------
-- Killmails, and the threat analysis derived from them
-- ---------------------------------------------------------------------------

-- Ingested from zKillboard's R2Z2 stream, which bundles the ESI body, so nothing here
-- costs a second request. `orgs` feeds the threat analysis; the rest is what a killmails
-- row displays. The detail columns are nullable and deliberately have no foreign keys:
-- these are ids from an external feed, and a ship type from a patch we have not seeded
-- yet must not stop a killmail being recorded.
create table killmails (
    id                        bigint primary key,
    hash                      text not null,
    solar_system_id           bigint not null,
    time                      timestamptz not null,
    -- Participating orgs, deduped per killmail:
    -- [{"id": ..., "kind": "alliance"|"corporation"}].
    orgs                      jsonb not null,
    victim_character_id       bigint,
    victim_corporation_id     bigint,
    victim_alliance_id        bigint,
    victim_ship_type_id       bigint,
    -- ISK. `double precision` rather than numeric: these are zKillboard's own estimates,
    -- accurate to a few percent at best, and only ever displayed rounded.
    total_value               double precision,
    attacker_count            integer,
    -- Killed by NPCs, and killed by exactly one attacker. Both change how a kill reads: an
    -- NPC kill in your chain means nothing, a solo kill means someone is hunting.
    is_npc                    boolean not null default false,
    is_solo                   boolean not null default false,
    final_blow_character_id   bigint,
    final_blow_corporation_id bigint,
    final_blow_alliance_id    bigint,
    final_blow_ship_type_id   bigint
);

-- The analysis scans one system over a window; the card scans a set of systems, newest
-- first. Neither index serves the other.
create index killmails_system_time on killmails (solar_system_id, time);
create index killmails_system_recent on killmails (solar_system_id, time desc);
create index killmails_time on killmails (time desc);

-- Cursor into zKillboard's R2Z2 sequence stream (single row).
create table zkb_state (
    id          boolean primary key default true,
    sequence_id bigint not null
);

create table wormhole_system_threats (
    id              bigint generated always as identity primary key,
    solar_system_id bigint not null
        references wormhole_systems (solar_system_id) on delete cascade,
    entity_id       bigint not null,
    entity_type     text not null,
    name            text not null,
    kills           int not null default 0,

    unique (solar_system_id, entity_type, entity_id)
);
