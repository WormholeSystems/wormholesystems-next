-- ---------------------------------------------------------------------------
-- Keeping a connection and its signatures in agreement
-- ---------------------------------------------------------------------------
--
-- A wormhole's life-cycle state lives on BOTH the connection and its signature(s), because
-- either can exist without the other: a connection can be drawn before anyone scans, and a
-- wormhole signature carries mass/EOL/size from the scanner before it is ever linked. Once
-- they are linked, every member of the "group" -- the connection plus its <=2 signatures
-- (one per endpoint system) -- must agree on (mass_status, time_status, size). Two rules:
--
--   * MERGE on link: per field, the most-severe (worst) non-null value wins. A connection
--     marked "<4h" (eol) linked to a sig scanned "<1h" (critical) becomes critical.
--   * PROPAGATE on edit: an explicit edit to any member overwrites the whole group
--     verbatim, so corrections and downgrades (e.g. back to stable) flow through.
--
-- Because a linked group is always fully consistent (every merge/propagate equalises all
-- three fields), propagating all three on a single-field edit is safe -- the untouched two
-- already match groupwide and the IS DISTINCT FROM guard skips them.
--
-- Severity order (worst last): mass stable<reduced<critical; time stable<eol<critical;
-- size xl<large<medium<small (smallest = most restrictive = "worst"). Mirrors the ordered
-- Rust enums in src/maps/mod.rs.

-- Severity rank of each enum value. NULL (unknown) ranks as NULL so it is ignored when
-- picking the worst non-null value, and a known value always wins over unknown.
create or replace function map_mass_rank(s text) returns int immutable language sql as $$
    select case s when 'stable' then 0 when 'reduced' then 1 when 'critical' then 2 end;
$$;
create or replace function map_time_rank(s text) returns int immutable language sql as $$
    select case s when 'stable' then 0 when 'eol' then 1 when 'critical' then 2 end;
$$;
create or replace function map_size_rank(s text) returns int immutable language sql as $$
    select case s when 'xl' then 0 when 'large' then 1 when 'medium' then 2 when 'small' then 3 end;
$$;

-- Overwrite every member of a connection's group with the given state, but only rows that
-- actually differ. The IS DISTINCT FROM guard is what makes the cascading triggers
-- terminate: once everyone equals the target, the recursive updates find nothing to do.
create or replace function map_sync_propagate(conn_id bigint, m text, t text, z text)
returns void language plpgsql as $$
begin
    update map_connections
       set mass_status = m, time_status = t, size = z, updated_at = now()
     where id = conn_id
       and (mass_status is distinct from m or time_status is distinct from t or size is distinct from z);
    update signatures
       set mass_status = m, time_status = t, size = z, updated_at = now()
     where connection_id = conn_id
       and (mass_status is distinct from m or time_status is distinct from t or size is distinct from z);
end;
$$;

-- Merge the group into the worst non-null value per field, then equalise everyone to it.
create or replace function map_sync_merge(conn_id bigint)
returns void language plpgsql as $$
declare
    m text; t text; z text;
begin
    with members as (
        select mass_status, time_status, size from map_connections where id = conn_id
        union all
        select mass_status, time_status, size from signatures where connection_id = conn_id
    )
    select
        (select mass_status from members where mass_status is not null order by map_mass_rank(mass_status) desc limit 1),
        (select time_status from members where time_status is not null order by map_time_rank(time_status) desc limit 1),
        (select size        from members where size        is not null order by map_size_rank(size)        desc limit 1)
      into m, t, z;
    perform map_sync_propagate(conn_id, m, t, z);
end;
$$;

-- A signature change: linking (NULL/other -> a connection) merges into that group;
-- editing mass/time/size while linked propagates verbatim. Unlinking needs no sync --
-- the remaining members keep their state.
create or replace function map_sig_sync() returns trigger language plpgsql as $$
begin
    if tg_op = 'INSERT' then
        if new.connection_id is not null then
            perform map_sync_merge(new.connection_id);
        end if;
    elsif tg_op = 'UPDATE' then
        if new.connection_id is distinct from old.connection_id then
            if new.connection_id is not null then
                perform map_sync_merge(new.connection_id);
            end if;
        elsif new.connection_id is not null
              and (new.mass_status is distinct from old.mass_status
                   or new.time_status is distinct from old.time_status
                   or new.size        is distinct from old.size) then
            perform map_sync_propagate(new.connection_id, new.mass_status, new.time_status, new.size);
        end if;
    end if;
    return null;
end;
$$;

create or replace trigger map_sig_sync after insert or update on signatures
    for each row execute function map_sig_sync();

-- A connection edit propagates its state to its linked signatures.
create or replace function map_conn_sync() returns trigger language plpgsql as $$
begin
    if new.mass_status is distinct from old.mass_status
       or new.time_status is distinct from old.time_status
       or new.size        is distinct from old.size then
        perform map_sync_propagate(new.id, new.mass_status, new.time_status, new.size);
    end if;
    return null;
end;
$$;

create or replace trigger map_conn_sync after update on map_connections
    for each row execute function map_conn_sync();

-- Stamp the lifetime change on either side, whichever path did the writing.
create or replace function map_stamp_time_status() returns trigger language plpgsql as $$
begin
    if new.time_status is distinct from old.time_status then
        new.time_status_updated_at = now();
    end if;
    return new;
end;
$$;

create trigger signatures_stamp_time_status
    before update on signatures
    for each row execute function map_stamp_time_status();

create trigger map_connections_stamp_time_status
    before update on map_connections
    for each row execute function map_stamp_time_status();
