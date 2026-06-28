-- The map graph + access control. See docs/database/mapping.md and access.md.
-- Enum-like columns (status, group, size, mass/time status, connection type, access
-- role/subject) are text, validated by Rust enums rather than PG enum types — so adding
-- a value is a code change, not a migration.

create table maps (
    id          bigint generated always as identity primary key,
    name        text not null,
    description text,
    image_url   text,
    created_at  timestamptz not null default now()
);

-- Ephemeral placement: a system as currently on the map.
create table map_solar_systems (
    id              bigint generated always as identity primary key,
    map_id          bigint not null,
    solar_system_id bigint not null,
    position_x      double precision not null,
    position_y      double precision not null,
    alias           text,
    -- A map has at most one home system (partial unique index below) but any number of
    -- pinned systems. Pinned systems are drag-locked and survive "clear map".
    is_home         boolean not null default false,
    is_pinned       boolean not null default false,
    created_at      timestamptz not null default now(),

    unique (map_id, solar_system_id),
    foreign key (map_id) references maps (id) on delete cascade,
    foreign key (solar_system_id) references solar_systems (id)
);

-- At most one home system per map.
create unique index map_solar_systems_one_home
    on map_solar_systems (map_id) where is_home;

-- Persisted intel: survives a system being removed from the map.
create table map_solar_system_details (
    id              bigint generated always as identity primary key,
    map_id          bigint not null,
    solar_system_id bigint not null,
    status          text not null default 'unscanned',
    occupying_group text,
    updated_at      timestamptz not null default now(),

    unique (map_id, solar_system_id),
    foreign key (map_id) references maps (id) on delete cascade,
    foreign key (solar_system_id) references solar_systems (id)
);

create table map_connections (
    id          bigint generated always as identity primary key,
    map_id      bigint not null,
    from_system bigint not null,
    to_system   bigint not null,
    type        text not null,
    created_at  timestamptz not null default now(),

    check (from_system <> to_system),
    foreign key (map_id) references maps (id) on delete cascade,
    foreign key (from_system) references map_solar_systems (id) on delete cascade,
    foreign key (to_system) references map_solar_systems (id) on delete cascade
);

create table signatures (
    id              bigint generated always as identity primary key,
    map_id          bigint not null,
    solar_system_id bigint not null,
    signature_id    text not null,
    "group"         text not null,
    name            text,
    size            text,
    mass_status     text,
    time_status     text,
    connection_id   bigint,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now(),

    unique (map_id, solar_system_id, signature_id),
    -- Tied to the placement, so removing a system deletes its signatures.
    foreign key (map_id, solar_system_id)
        references map_solar_systems (map_id, solar_system_id) on delete cascade,
    foreign key (connection_id) references map_connections (id) on delete set null
);

-- subject_id is a character / corporation / alliance EVE id (polymorphic per
-- subject_type), so it can't FK a single table.
create table map_access (
    id           bigint generated always as identity primary key,
    map_id       bigint not null,
    subject_type text not null,
    subject_id   bigint not null,
    role         text not null,
    created_at   timestamptz not null default now(),

    unique (map_id, subject_id),
    foreign key (map_id) references maps (id) on delete cascade
);
