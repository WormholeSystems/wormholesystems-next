-- Threat analysis: locally-ingested killmails (minimal rows: the participating
-- organisations, not the full ESI payload) and the per-wormhole-system analysis results.

create table killmails (
    id              bigint primary key,
    hash            text not null,
    solar_system_id bigint not null,
    time            timestamptz not null,
    -- Participating orgs, deduped per killmail: [{"id": ..., "kind": "alliance"|"corporation"}].
    orgs            jsonb not null
);

create index killmails_system_time on killmails (solar_system_id, time);

-- Cursor into zKillboard's R2Z2 sequence stream (single row).
create table zkb_state (
    id          boolean primary key default true,
    sequence_id bigint not null
);

alter table wormhole_systems add column threat_level text not null default 'unknown';
alter table wormhole_systems add column threat_analyzed_at timestamptz;

create table wormhole_system_threats (
    id              bigint generated always as identity primary key,
    solar_system_id bigint not null,
    entity_id       bigint not null,
    entity_type     text not null,
    name            text not null,
    kills           int not null default 0,

    unique (solar_system_id, entity_type, entity_id),
    foreign key (solar_system_id) references wormhole_systems (solar_system_id) on delete cascade
);
