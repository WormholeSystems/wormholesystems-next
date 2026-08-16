-- Jump tracking for wormhole connections: every observed or manually logged transit,
-- with the ship's hull mass, feeding the connection's mass-remaining estimate.
-- `connection_id` null = a pending row observed before the connection was mapped; it is
-- claimed by a matching connection created shortly after, or pruned.
create table map_connection_jumps (
    id                  bigint generated always as identity primary key,
    map_id              bigint not null references maps (id) on delete cascade,
    connection_id       bigint references map_connections (id) on delete cascade,
    -- Kept when the character disappears, so the mass ledger survives.
    character_id        bigint references characters (id) on delete set null,
    from_solar_system_id bigint not null references solar_systems (id),
    to_solar_system_id   bigint not null references solar_systems (id),
    ship_type_id        bigint references types (id) on delete set null,
    ship_name           text,
    mass                bigint not null default 0,
    is_manual           boolean not null default false,
    created_at          timestamptz not null default now(),
    updated_at          timestamptz not null default now()
);

create index map_connection_jumps_claim
    on map_connection_jumps (map_id, from_solar_system_id, to_solar_system_id, created_at);
create index map_connection_jumps_by_connection
    on map_connection_jumps (connection_id, created_at);
