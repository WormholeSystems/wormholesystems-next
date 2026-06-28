-- A map's rally point: at most one per map, like the home system. Added as its own migration
-- (rather than editing 0005) so already-applied databases don't fail the startup checksum check.
-- `if not exists` makes it a no-op where the column was added manually during development.

alter table map_solar_systems
    add column if not exists is_rally boolean not null default false;

create unique index if not exists map_solar_systems_one_rally
    on map_solar_systems (map_id) where is_rally;
