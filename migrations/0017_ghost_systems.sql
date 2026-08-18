-- Ghost placements: the far side of a wormhole signature, before anyone knows what it is.
--
-- See docs/database/mapping.md#ghost-placements. A ghost is an ordinary placement with no
-- solar system, so connections, positions and aliases work on it unchanged. `unique
-- (map_id, solar_system_id)` still caps real systems at one per map: nulls are distinct.
alter table map_solar_systems alter column solar_system_id drop not null;

-- Home and rally both mean a place you can go, which a ghost is not yet.
alter table map_solar_systems
    add constraint map_solar_systems_ghost_unmarked
    check (solar_system_id is not null or not (is_home or is_rally));

-- Whether pasting a wormhole signature puts its far side on the map. Map-wide: a ghost is
-- a node everyone on the chain sees, so it cannot be one person's preference.
alter table maps add column ghost_unlinked_wormholes boolean not null default false;
