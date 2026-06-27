-- Records which SDE build is currently loaded into the reference tables, so startup
-- can skip the (large) re-seed when nothing has changed. See docs/database/seeding.md.
--
-- Single-row table: the boolean primary key defaults to true and is checked, so a
-- second row can never be inserted — there is only ever one "currently loaded" build.

create table sde_build (
    id           boolean primary key default true,
    build_number bigint not null,
    release_date timestamptz,
    loaded_at    timestamptz not null default now(),

    check (id)
);
