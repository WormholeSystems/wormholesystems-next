-- ---------------------------------------------------------------------------
-- Tranquility itself
-- ---------------------------------------------------------------------------

-- What the server is doing right now.
--
-- One row, not a history: the only question anyone asks is "is it up", and a row a minute
-- would be a third of a million rows a year to answer it. It is persisted at all so that a
-- restart does not blank the header until the first poll lands, and so the gate on the ESI
-- pollers survives one too.
create table server_status (
    -- The singleton lock: `check (id)` allows only `true`, so a second row cannot exist.
    id             boolean primary key default true check (id),
    -- Whether the last poll reached ESI at all. Distinguishes "Tranquility is down" from
    -- "we cannot tell", which are different problems with the same symptom.
    reachable      boolean not null,
    players        bigint not null default 0,
    server_version text,
    start_time     timestamptz,
    -- Up, but only CCP can log in.
    vip            boolean not null default false,
    checked_at     timestamptz not null default now()
);
