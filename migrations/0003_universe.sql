-- ---------------------------------------------------------------------------
-- Universe: SDE topology, ESI-cached entities, and the sovereignty overlay
-- ---------------------------------------------------------------------------

create table factions (
    id                     bigint primary key,
    name                   text not null,
    description            text,
    corporation_id         bigint,
    militia_corporation_id bigint,
    home_solar_system_id   bigint,
    size_factor            double precision
);

create table corporations (
    id           bigint primary key,
    name         text not null,
    ticker       text not null,
    alliance_id  bigint,
    faction_id   bigint,
    ceo_id       bigint,
    member_count integer,
    updated_at   timestamptz not null default now()
);

create table alliances (
    id                      bigint primary key,
    name                    text not null,
    ticker                  text not null,
    creator_corporation_id  bigint,
    executor_corporation_id bigint,
    faction_id              bigint,
    updated_at              timestamptz not null default now()
);

create table regions (
    id                bigint primary key,
    name              text not null,
    faction_id        bigint references factions (id),
    wormhole_class_id integer
);

create table constellations (
    id         bigint primary key,
    region_id  bigint not null references regions (id),
    name       text not null,
    faction_id bigint references factions (id)
);

create table solar_systems (
    id                bigint primary key,
    constellation_id  bigint not null references constellations (id),
    region_id         bigint not null references regions (id),
    name              text not null,
    security_status   double precision not null,
    security_class    text,
    faction_id        bigint references factions (id),
    wormhole_class_id integer,
    star_id           bigint
);

create table stargates (
    id                      bigint primary key,
    solar_system_id         bigint not null references solar_systems (id),
    destination_system_id   bigint not null references solar_systems (id),
    destination_stargate_id bigint not null,
    type_id                 bigint not null references types (id)
);

create table planets (
    id              bigint primary key,
    solar_system_id bigint not null references solar_systems (id),
    type_id         bigint not null references types (id),
    celestial_index integer not null,
    name            text
);

create table moons (
    id              bigint primary key,
    solar_system_id bigint not null references solar_systems (id),
    type_id         bigint not null references types (id),
    celestial_index integer not null,
    name            text
);

create table asteroid_belts (
    id              bigint primary key,
    solar_system_id bigint not null references solar_systems (id),
    type_id         bigint not null references types (id),
    celestial_index integer not null,
    name            text
);

create table stations (
    id                   bigint primary key,
    solar_system_id      bigint not null references solar_systems (id),
    type_id              bigint not null references types (id),
    owner_corporation_id bigint,
    operation_id         bigint,
    name                 text
);

create table structures (
    id                   bigint primary key,
    solar_system_id      bigint references solar_systems (id),
    name                 text,
    type_id              bigint references types (id),
    owner_corporation_id bigint,
    updated_at           timestamptz not null default now()
);

create table system_sovereignty (
    solar_system_id   bigint primary key references solar_systems (id),
    alliance_id       bigint,
    corporation_id    bigint,
    faction_id        bigint references factions (id),
    claimed_since     timestamptz,
    is_capital_system boolean,
    updated_at        timestamptz not null default now()
);

-- Station services (SDE stationServices plus the per-operation service sets), so the
-- navigation Find can answer "nearest system with repair / cloning / ...".
create table station_services (
    id   bigint primary key,
    name text not null
);

create table station_operation_services (
    operation_id bigint not null,
    service_id   bigint not null references station_services (id),
    primary key (operation_id, service_id)
);

-- The entity cross-references, added once every table exists. They span a
-- corporations <-> alliances <-> factions cycle, so they cannot be inline.
--
-- `map_access.subject_id` stays unconstrained on purpose: it is polymorphic per
-- `subject_type`, so it cannot reference a single table.
alter table characters
    add foreign key (corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (alliance_id) references alliances (id) deferrable initially deferred;

alter table factions
    add foreign key (corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (militia_corporation_id) references corporations (id) deferrable initially deferred;

alter table corporations
    add foreign key (alliance_id) references alliances (id) deferrable initially deferred,
    add foreign key (faction_id) references factions (id) deferrable initially deferred;

alter table alliances
    add foreign key (creator_corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (executor_corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (faction_id) references factions (id) deferrable initially deferred;

alter table stations
    add foreign key (owner_corporation_id) references corporations (id) deferrable initially deferred;

alter table structures
    add foreign key (owner_corporation_id) references corporations (id) deferrable initially deferred;

alter table system_sovereignty
    add foreign key (alliance_id) references alliances (id) deferrable initially deferred,
    add foreign key (corporation_id) references corporations (id) deferrable initially deferred;
