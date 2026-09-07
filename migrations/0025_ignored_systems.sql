-- Systems the map as a whole keeps out of the way: the router steers around them for
-- everyone, and mapping as you fly never places them. Shared, unlike the viewer's own
-- route-around list, which stays in the browser.
create table map_ignored_solar_systems (
    id              bigint generated always as identity primary key,
    map_id          bigint not null references maps (id) on delete cascade,
    solar_system_id bigint not null references solar_systems (id),
    created_at      timestamptz not null default now(),

    unique (map_id, solar_system_id)
);
