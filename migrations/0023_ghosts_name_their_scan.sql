-- A ghost is the far side of one scanned hole, hanging off the system it was scanned in.
-- Both of those were only ever implied: the signature through its connection, the system
-- through the same edge. Naming them makes the two rules that govern a ghost's life
-- ("the scan is gone, so is the hole" and "the system is gone, so is what hung off it")
-- foreign keys, true no matter which write removed the row.

-- Deferred, because restoring a removal walks a cycle: a ghost names its signature, a
-- signature names its connection, and a connection names its endpoints. Checking at commit
-- lets the undo put the three back in any order and still be right at the end of it.
alter table map_solar_systems
    add column raised_by_signature_id bigint
        references signatures (id) on delete cascade deferrable initially deferred,
    add column hangs_off_id bigint
        references map_solar_systems (id) on delete cascade deferrable initially deferred;

update map_solar_systems g
set raised_by_signature_id = s.id,
    hangs_off_id = case when c.from_system = g.id then c.to_system else c.from_system end
from map_connections c
join signatures s on s.connection_id = c.id
where g.solar_system_id is null
  and c.map_id = g.map_id
  and (c.from_system = g.id or c.to_system = g.id);

-- Ghosts no scan claims never had anything to be; there is nothing to backfill them with.
delete from map_solar_systems where solar_system_id is null and raised_by_signature_id is null;

-- The backfill above queued a deferred check for every row it touched, and Postgres will
-- not alter a table with trigger events still pending. Settling them here is what lets the
-- constraint go on in the same transaction.
set constraints all immediate;

-- A node is either a system somebody placed or a hole somebody scanned, never both and
-- never neither.
alter table map_solar_systems
    add constraint map_solar_systems_ghost_names_its_scan
    check (
        (solar_system_id is not null
             and raised_by_signature_id is null and hangs_off_id is null)
        or (solar_system_id is null
             and raised_by_signature_id is not null and hangs_off_id is not null)
    );

create index map_solar_systems_raised_by_idx
    on map_solar_systems (raised_by_signature_id)
    where raised_by_signature_id is not null;
