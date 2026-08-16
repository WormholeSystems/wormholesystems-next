-- The map command journal: one row per applied mutation, recorded in the same
-- transaction as the write it describes. `inverse` is a serialized MapCommand, so
-- undo is just another execution (and redo is undoing the undo).
create table map_events (
    id            bigint generated always as identity primary key,
    map_id        bigint not null references maps (id) on delete cascade,
    -- null = applied by a background task (expiry, jump capture).
    character_id  bigint references characters (id) on delete set null,
    kind          text not null,
    label         text not null,
    entries_count int not null default 1,
    inverse       jsonb,
    undone_at     timestamptz,
    reverts_id    bigint references map_events (id) on delete set null,
    created_at    timestamptz not null default now()
);

create index map_events_by_map on map_events (map_id, created_at desc);
