-- ---------------------------------------------------------------------------
-- Static reference, seeded from data/static/*.json
-- ---------------------------------------------------------------------------

create table wormhole_types (
    code               text primary key,
    type_id            bigint not null references types (id),
    dest_class         integer,
    is_static          boolean,
    max_mass_per_jump  bigint,
    total_mass         bigint,
    mass_regen         bigint,
    lifetime_hours     double precision,
    signature_strength double precision,
    sibling_groups     jsonb
);

create table wormhole_type_sources (
    wormhole_code     text not null references wormhole_types (code) on delete cascade,
    wormhole_class_id integer not null,
    primary key (wormhole_code, wormhole_class_id)
);

create table wormhole_effects (
    name text primary key
);

create table wormhole_effect_modifiers (
    effect_name       text not null references wormhole_effects (name) on delete cascade,
    kind              text not null,
    stat              text not null,
    wormhole_class_id integer not null,
    value             text not null,
    primary key (effect_name, kind, stat, wormhole_class_id)
);

-- How busy a wormhole is with other people's killmails. Declared worst-last, which is the
-- order Postgres compares an enum in.
create type threat_level as enum ('unknown', 'high', 'critical');

-- `threat_level` and `threat_analyzed_at` are the output of the daily killmail analysis
-- further down, cached here because every map node reads them.
create table wormhole_systems (
    solar_system_id    bigint primary key references solar_systems (id),
    wormhole_class_id  integer not null,
    effect_name        text references wormhole_effects (name),
    is_shattered       boolean not null default false,
    threat_level       threat_level not null default 'unknown',
    threat_analyzed_at timestamptz
);

create table wormhole_system_statics (
    solar_system_id bigint not null references wormhole_systems (solar_system_id) on delete cascade,
    wormhole_code   text not null references wormhole_types (code),
    primary key (solar_system_id, wormhole_code)
);

-- Signature catalogue. Ids come from the source JSON, so they are not generated.
create table signature_categories (
    id   bigint primary key,
    name text not null,
    code text not null unique
);

create table signature_types (
    id                    bigint primary key,
    signature             text,
    name                  text not null,
    signature_category_id bigint not null references signature_categories (id),
    target_class          integer,
    extra                 text
);

create table signature_type_spawn_areas (
    signature_type_id bigint not null references signature_types (id) on delete cascade,
    wormhole_class_id integer not null,
    primary key (signature_type_id, wormhole_class_id)
);

create table jove_observatories (
    solar_system_id bigint primary key references solar_systems (id)
);

-- Which SDE build is currently loaded, so startup can skip the (large) re-seed when
-- nothing has changed. `seed_revision` bumps when the *format* changes rather than the
-- source data. Single-row table: the boolean primary key defaults to true and is checked,
-- so a second row can never exist.
create table sde_build (
    id            boolean primary key default true,
    build_number  bigint not null,
    release_date  timestamptz,
    seed_revision int not null default 0,
    loaded_at     timestamptz not null default now(),

    check (id)
);
