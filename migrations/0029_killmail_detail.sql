-- What died, and what it was worth.
--
-- The ingest already receives all of this from zKillboard and was discarding it: the table
-- existed only to feed threat analysis, which needs no more than who was involved, where
-- and when. Showing recent kills needs the rest, and none of it costs another request.
--
-- Nullable throughout, and deliberately without foreign keys. These are ids from an
-- external feed: a ship type from a patch we have not seeded yet, or a character we have
-- never seen, must not stop a killmail being recorded.
alter table killmails
    add column victim_character_id   bigint,
    add column victim_corporation_id bigint,
    add column victim_alliance_id    bigint,
    add column victim_ship_type_id   bigint,
    -- ISK. `double precision` rather than numeric: these are zKillboard's own estimates,
    -- accurate to a few percent at best, and they are only ever displayed rounded.
    add column total_value           double precision,
    add column attacker_count        integer,
    -- Killed by NPCs, and killed by exactly one attacker. Both change how a kill reads:
    -- an NPC kill in your chain means nothing, a solo kill means someone is hunting.
    add column is_npc                boolean not null default false,
    add column is_solo               boolean not null default false,
    add column final_blow_character_id   bigint,
    add column final_blow_corporation_id bigint,
    add column final_blow_alliance_id    bigint,
    add column final_blow_ship_type_id   bigint;

-- The card asks "what has died recently, anywhere on this map", which is a scan back
-- through time across a set of systems.
create index killmails_time on killmails (time desc);
