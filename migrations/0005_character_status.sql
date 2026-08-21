-- ---------------------------------------------------------------------------
-- Live character presence, polled from ESI
-- ---------------------------------------------------------------------------

create table character_status (
    character_id    bigint primary key references characters (id) on delete cascade,
    solar_system_id bigint references solar_systems (id),
    station_id      bigint,
    structure_id    bigint,
    is_docked       boolean generated always as
        (station_id is not null or structure_id is not null) stored,
    online          boolean not null default false,
    last_online_at  timestamptz,
    ship_type_id    bigint references types (id),
    ship_name       text,
    -- The hull itself, not its type: the same pilot in a second Loki is a different item.
    -- Kept so `ship_updated_at` can say how long they have been in this one, which is the
    -- difference between somebody who just undocked and somebody who has been sitting on a
    -- hole for an hour.
    ship_item_id    bigint,
    ship_updated_at timestamptz,
    updated_at      timestamptz not null default now()
);

-- Deferred, like the other entity references: a structure may not be resolved yet when
-- the location that mentions it is written.
alter table character_status
    add foreign key (station_id) references stations (id) deferrable initially deferred,
    add foreign key (structure_id) references structures (id) deferrable initially deferred;
