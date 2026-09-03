-- ---------------------------------------------------------------------------
-- An identified wormhole type dictates the connection's size
-- ---------------------------------------------------------------------------
--
-- A wormhole type's maximum jump mass is physics, not preference: nothing bigger than a
-- cruiser fits through a C247 whatever anyone picks in a menu. So once any signature linked
-- to a connection carries a type with a jump mass (K162 carries none), the group's `size`
-- is that type's size, and every write path ends up there: linking, retyping, pasting, and
-- manual size edits on either the connection or a signature. When no linked signature
-- identifies the hole any more the lock lifts and the group keeps the size it has.
--
-- The lock lives in map_sync_propagate, the single write point of the sync in 0007, so
-- every merge and propagation passes through it. Retyping a linked signature is the one
-- edit 0007's trigger ignored, so map_sig_sync now propagates on that too.

-- The size a hole admits by its maximum jump mass. The thresholds mirror `sizeForJumpMass`
-- in the frontend.
create or replace function map_size_for_jump_mass(kg bigint)
returns wormhole_size language sql immutable as $$
    select case
        when kg is null then null
        when kg <= 5000000 then 'small'::wormhole_size
        when kg <= 300000000 then 'medium'::wormhole_size
        when kg <= 1000000000 then 'large'::wormhole_size
        else 'xl'::wormhole_size
    end;
$$;

-- The size a connection's linked signatures dictate, or null when none identifies the hole.
-- Two identifying signatures should never disagree, but if they do the most restrictive
-- wins, like the merge in 0007.
create or replace function map_locked_size(conn_id bigint)
returns wormhole_size language sql stable as $$
    select map_size_for_jump_mass(w.max_mass_per_jump) as size
      from signatures s
      join signature_types st on st.id = s.signature_type_id
      join wormhole_types w on w.code = st.signature
     where s.connection_id = conn_id
       and w.max_mass_per_jump is not null
     order by size desc
     limit 1;
$$;

-- As in 0007, but the requested size yields to the locked one.
create or replace function map_sync_propagate(conn_id bigint, m mass_status, t time_status, z wormhole_size)
returns void language plpgsql as $$
declare
    locked wormhole_size := coalesce(map_locked_size(conn_id), z);
begin
    update map_connections
       set mass_status = m, time_status = t, size = locked, updated_at = now()
     where id = conn_id
       and (mass_status is distinct from m or time_status is distinct from t or size is distinct from locked);
    update signatures
       set mass_status = m, time_status = t, size = locked, updated_at = now()
     where connection_id = conn_id
       and (mass_status is distinct from m or time_status is distinct from t or size is distinct from locked);
end;
$$;

-- As in 0007, plus: retyping a linked signature propagates, so the group picks up (or
-- drops) the lock its new type carries.
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
              and (new.mass_status       is distinct from old.mass_status
                   or new.time_status       is distinct from old.time_status
                   or new.size              is distinct from old.size
                   or new.signature_type_id is distinct from old.signature_type_id) then
            perform map_sync_propagate(new.connection_id, new.mass_status, new.time_status, new.size);
        end if;
    end if;
    return null;
end;
$$;
