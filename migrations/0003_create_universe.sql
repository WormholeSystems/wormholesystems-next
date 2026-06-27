-- EVE universe: SDE topology + ESI-cached entities + dynamic overlays.
-- See docs/database/universe.md. Runs after item types (0002) so `type_id` resolves.
--
-- Entities/factions are created first so the topology's `type_id` and `faction_id` FKs
-- resolve (both are pure SDE data, loaded ahead of the topology). `corporation_id` /
-- `alliance_id` stay plain bigint: they reference ESI-cached entities loaded
-- independently of the SDE (and would dangle / form a corporations<->alliances<->factions
-- cycle), so a hard FK would couple unrelated loads.

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
    region_id  bigint not null,
    name       text not null,
    faction_id bigint,

    foreign key (region_id) references regions (id),
    foreign key (faction_id) references factions (id)
);

create table solar_systems (
    id                bigint primary key,
    constellation_id  bigint not null,
    region_id         bigint not null,
    name              text not null,
    security_status   double precision not null,
    security_class    text,
    faction_id        bigint,
    wormhole_class_id integer,
    star_id           bigint,

    foreign key (constellation_id) references constellations (id),
    foreign key (region_id) references regions (id),
    foreign key (faction_id) references factions (id)
);

create table stargates (
    id                      bigint primary key,
    solar_system_id         bigint not null,
    destination_system_id   bigint not null,
    destination_stargate_id bigint not null,
    type_id                 bigint not null,

    foreign key (solar_system_id) references solar_systems (id),
    foreign key (destination_system_id) references solar_systems (id),
    foreign key (type_id) references types (id)
);

create table planets (
    id              bigint primary key,
    solar_system_id bigint not null,
    type_id         bigint not null,
    celestial_index integer not null,
    name            text,

    foreign key (solar_system_id) references solar_systems (id),
    foreign key (type_id) references types (id)
);

create table moons (
    id              bigint primary key,
    solar_system_id bigint not null,
    type_id         bigint not null,
    celestial_index integer not null,
    name            text,

    foreign key (solar_system_id) references solar_systems (id),
    foreign key (type_id) references types (id)
);

create table asteroid_belts (
    id              bigint primary key,
    solar_system_id bigint not null,
    type_id         bigint not null,
    celestial_index integer not null,
    name            text,

    foreign key (solar_system_id) references solar_systems (id),
    foreign key (type_id) references types (id)
);

create table stations (
    id                   bigint primary key,
    solar_system_id      bigint not null,
    type_id              bigint not null,
    owner_corporation_id bigint,
    operation_id         bigint,
    name                 text,

    foreign key (solar_system_id) references solar_systems (id),
    foreign key (type_id) references types (id)
);

create table structures (
    id                   bigint primary key,
    solar_system_id      bigint,
    name                 text,
    type_id              bigint,
    owner_corporation_id bigint,
    updated_at           timestamptz not null default now(),

    foreign key (solar_system_id) references solar_systems (id),
    foreign key (type_id) references types (id)
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
