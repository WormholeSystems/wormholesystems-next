-- ---------------------------------------------------------------------------
-- Item types (SDE): categories -> groups -> types, plus the dogma catalogue
-- ---------------------------------------------------------------------------

create table categories (
    id        bigint primary key,
    name      text not null,
    published boolean not null default false
);

create table groups (
    id          bigint primary key,
    category_id bigint not null references categories (id),
    name        text not null,
    published   boolean not null default false
);

create table market_groups (
    id              bigint primary key,
    -- Self-referential; deferrable so a bulk seed needn't order parents before children.
    parent_group_id bigint references market_groups (id) deferrable initially deferred,
    name            text not null,
    has_types       boolean not null default false
);

create table types (
    id              bigint primary key,
    group_id        bigint not null references groups (id),
    market_group_id bigint references market_groups (id),
    name            text not null,
    published       boolean not null default false,
    volume          double precision,
    mass            double precision,
    capacity        double precision,
    icon_id         bigint
);

create table dogma_units (
    id   bigint primary key,
    name text not null
);

create table dogma_attributes (
    id            bigint primary key,
    name          text not null,
    unit_id       bigint references dogma_units (id),
    default_value double precision not null default 0,
    high_is_good  boolean not null default false,
    published     boolean not null default false
);

create table type_attributes (
    type_id      bigint not null references types (id) on delete cascade,
    attribute_id bigint not null references dogma_attributes (id),
    value        double precision not null,
    primary key (type_id, attribute_id)
);
