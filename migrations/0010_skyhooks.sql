-- ---------------------------------------------------------------------------
-- Skyhooks
-- ---------------------------------------------------------------------------

-- Skyhooks whose theft window is open or about to open.
--
-- A mirror of what ESI is currently publishing, not a log: the endpoint only ever returns
-- the raidable set, so a row that stops being returned has stopped being raidable and is
-- deleted. Nothing here is historical, and nothing is per-map -- a raidable skyhook is the
-- same fact for everyone.
create table raidable_skyhooks (
    -- The planet is the skyhook's identity; there is one per planet.
    planet_id        bigint primary key references planets (id),
    solar_system_id  bigint not null references solar_systems (id),
    vulnerable_from  timestamptz not null,
    vulnerable_until timestamptz not null,
    updated_at       timestamptz not null default now()
);

-- Every query is "what is still open", which reads this.
create index raidable_skyhooks_until on raidable_skyhooks (vulnerable_until);
