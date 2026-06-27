-- Custom static reference, seeded from data/static/*.json. See docs/database/static.md.
-- Wormhole class ids are plain integers (the wormhole_class_id encoding). Enum-like
-- columns (e.g. `kind`) are text, validated by Rust enums rather than PG enum types.

create table wormhole_types (
    code              text primary key,
    type_id           bigint not null references types (id),
    dest_class        integer,
    is_static         boolean,
    max_mass_per_jump bigint,
    total_mass        bigint,
    mass_regen        bigint,
    lifetime_hours    integer,
    sibling_groups    jsonb
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

create table wormhole_systems (
    solar_system_id   bigint primary key references solar_systems (id),
    wormhole_class_id integer not null,
    effect_name       text references wormhole_effects (name)
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
    signature             text not null,
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
