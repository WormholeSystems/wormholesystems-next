-- Navigation panel: the per-map watchlist and per-user route-calculation settings.

-- Systems whose distance is tracked on a map (legacy map_route_solarsystems).
create table map_watchlist (
    id              bigint generated always as identity primary key,
    map_id          bigint not null references maps (id) on delete cascade,
    solar_system_id bigint not null references solar_systems (id),
    is_pinned       boolean not null default false,
    created_at      timestamptz not null default now(),

    unique (map_id, solar_system_id)
);

-- Route calculation settings (legacy defaults). The tolerance columns store the worst
-- status still allowed to route through: time 'critical' = anything, mass 'reduced' =
-- fresh or reduced holes only.
alter table map_user_settings
    add column route_preference text not null default 'shorter',
    add column security_penalty int not null default 50,
    add column route_allow_time_status text not null default 'critical',
    add column route_allow_mass_status text not null default 'reduced',
    add column route_use_evescout boolean not null default false;
