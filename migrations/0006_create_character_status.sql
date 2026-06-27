-- Live character presence, polled from ESI. See docs/database/tracking.md. 1:1 per
-- character. station_id / structure_id stay plain (ESI location ids; a structure may
-- not be resolved yet).

create table character_status (
    character_id    bigint primary key,
    solar_system_id bigint,
    station_id      bigint,
    structure_id    bigint,
    is_docked       boolean generated always as
        (station_id is not null or structure_id is not null) stored,
    online          boolean not null default false,
    last_online_at  timestamptz,
    ship_type_id    bigint,
    ship_name       text,
    ship_item_id    bigint,
    ship_updated_at timestamptz,
    updated_at      timestamptz not null default now(),

    foreign key (character_id) references characters (id) on delete cascade,
    foreign key (solar_system_id) references solar_systems (id),
    foreign key (ship_type_id) references types (id)
);
