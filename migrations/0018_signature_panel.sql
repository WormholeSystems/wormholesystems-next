-- Signature panel parity: catalog-typed signatures, lifetime timestamps, the
-- preserve-mass connection flag, and the panel's per-user settings.

-- A signature can reference a catalog type (`signature_types`, seeded from the static
-- data). `name` stays as the raw/unmatched type name from the scanner.
alter table signatures
    add column signature_type_id bigint references signature_types (id);

-- When the wormhole life-cycle (EOL / critical) was last changed, for "EOL since" ages.
-- Maintained by triggers so every write path is covered, including the 0009 sync.
alter table signatures add column time_status_updated_at timestamptz;
alter table map_connections add column time_status_updated_at timestamptz;

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

-- Legacy flag: exclude this connection's hole from mass bookkeeping. Stored and
-- toggleable; nothing consumes it until jump-mass tracking exists.
alter table map_connections add column preserve_mass boolean not null default false;

-- Signatures-panel preferences (legacy parity).
alter table map_user_settings
    add column compact_signature_list boolean not null default false,
    add column show_statics_first boolean not null default false;
