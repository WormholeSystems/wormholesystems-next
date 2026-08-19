-- Pinning is for a place you have decided matters: it drag-locks the node, roots the tree
-- layout, survives "clear map", and is skipped by every sweep. None of that is meaningful
-- for a hole nobody has been through, and the last one would let a ghost outlive the
-- connection it is the far side of.
alter table map_solar_systems drop constraint map_solar_systems_ghost_unmarked;
alter table map_solar_systems
    add constraint map_solar_systems_ghost_unmarked
    check (solar_system_id is not null or not (is_home or is_rally or is_pinned));
